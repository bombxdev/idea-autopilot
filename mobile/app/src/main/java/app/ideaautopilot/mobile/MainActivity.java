package app.ideaautopilot.mobile;

import android.app.Activity;
import android.content.Intent;
import android.database.Cursor;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Color;
import android.graphics.ImageDecoder;
import android.provider.OpenableColumns;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.util.Arrays;
import java.util.List;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.view.WindowInsets;
import android.widget.FrameLayout;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

/** A WebView shell around assets/index.html; all logic lives in the page. */
public class MainActivity extends Activity {
    private WebView web;
    private static final int PICK_FILES = 7;
    /** the page's <input type=file> waiting for the gallery / file picker */
    private ValueCallback<Uri[]> pending;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        web = new WebView(this);
        web.setBackgroundColor(Color.parseColor("#0f1218"));
        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        s.setAllowFileAccess(true);
        web.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest req) {
                Uri u = req.getUrl();
                if ("file".equals(u.getScheme())) return false;
                // links in reports open in the browser, not inside the app
                startActivity(new Intent(Intent.ACTION_VIEW, u));
                return true;
            }
        });
        // <input type=file accept="image/*"> → the system picker (gallery, photos, files); without this nothing opens
        web.setWebChromeClient(new WebChromeClient() {
            @Override
            public boolean onShowFileChooser(WebView view, ValueCallback<Uri[]> cb, FileChooserParams params) {
                if (pending != null) pending.onReceiveValue(null);
                pending = cb;
                Intent pick = new Intent(Intent.ACTION_GET_CONTENT);
                pick.addCategory(Intent.CATEGORY_OPENABLE);
                String[] types = params.getAcceptTypes();
                pick.setType(types != null && types.length > 0 && !types[0].isEmpty() ? types[0] : "*/*");
                pick.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, params.getMode() == FileChooserParams.MODE_OPEN_MULTIPLE);
                try {
                    startActivityForResult(Intent.createChooser(pick, "เลือกรูป"), PICK_FILES);
                } catch (Exception e) {
                    pending = null;
                    return false;
                }
                return true;
            }
        });
        // Android 15 draws apps edge-to-edge: keep the page clear of the status bar,
        // display cutout and navigation bar by padding a container to the real insets
        FrameLayout root = new FrameLayout(this);
        root.setBackgroundColor(Color.parseColor("#0f1218"));
        root.addView(web);
        root.setOnApplyWindowInsetsListener((v, insets) -> {
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets b = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                v.setPadding(b.left, b.top, b.right, b.bottom);
            } else {
                v.setPadding(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                        insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            }
            return insets;
        });
        setContentView(root);
        if (state != null) web.restoreState(state);
        else web.loadUrl("file:///android_asset/index.html");
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != PICK_FILES || pending == null) return;
        Uri[] picked = null;
        if (result == RESULT_OK && data != null) {
            if (data.getClipData() != null) {
                int n = data.getClipData().getItemCount();
                picked = new Uri[n];
                for (int i = 0; i < n; i++) picked[i] = data.getClipData().getItemAt(i).getUri();
            } else if (data.getData() != null) {
                picked = new Uri[] { data.getData() };
            }
        }
        // the page's browser engine cannot open HEIC/HEIF (iPhone and many Android cameras) or DNG; Android can,
        // so such photos are turned into JPEG here first (ISSUE-081). Off the UI thread: big photos take a moment.
        final Uri[] chosen = picked;
        final ValueCallback<Uri[]> cb = pending;
        pending = null;
        if (chosen == null) { cb.onReceiveValue(null); return; }
        new Thread(() -> {
            File dir = new File(getCacheDir(), "picked");
            dir.mkdirs();
            File[] old = dir.listFiles();
            if (old != null) for (File f : old) if (System.currentTimeMillis() - f.lastModified() > 3600_000L) f.delete();
            Uri[] out = new Uri[chosen.length];
            for (int i = 0; i < chosen.length; i++) out[i] = webReady(chosen[i], dir, i);
            runOnUiThread(() -> cb.onReceiveValue(out));
        }).start();
    }

    /** image types the page can open as they are */
    private static final List<String> WEB_IMAGES = Arrays.asList(
        "image/jpeg", "image/jpg", "image/png", "image/gif", "image/webp", "image/bmp", "image/x-ms-bmp", "image/avif", "image/svg+xml", "image/x-icon", "image/vnd.microsoft.icon");

    /** the picked file, or a JPEG copy of it when it is a photo the page cannot open (HEIC, HEIF, DNG…) */
    private Uri webReady(Uri uri, File dir, int n) {
        String type = getContentResolver().getType(uri);
        String name = displayName(uri);
        String lower = name.toLowerCase();
        boolean web = type != null && WEB_IMAGES.contains(type.toLowerCase())
            && !lower.endsWith(".heic") && !lower.endsWith(".heif");
        boolean image = (type != null && type.startsWith("image/")) || lower.matches(".*[.](heic|heif|dng|avif)$");
        if (web || !image) return uri;
        try {
            Bitmap bmp = decode(uri);
            if (bmp == null) return uri;
            String base = name.replaceAll("[.][^.]*$", "").replaceAll("[^A-Za-z0-9_-]+", "_");
            if (base.isEmpty()) base = "photo";
            File f = new File(dir, base + (n > 0 ? "_" + n : "") + ".jpg");
            try (FileOutputStream o = new FileOutputStream(f)) { bmp.compress(Bitmap.CompressFormat.JPEG, 90, o); }
            bmp.recycle();
            return Uri.fromFile(f);
        } catch (Throwable e) {
            return uri; // the page then says it cannot open this one
        }
    }

    /** decode at most ~2400 px on the long side (the page shrinks to 1600 anyway), turned upright */
    private Bitmap decode(Uri uri) throws Exception {
        final int max = 2400;
        if (Build.VERSION.SDK_INT >= 28) {
            ImageDecoder.Source src = ImageDecoder.createSource(getContentResolver(), uri);
            return ImageDecoder.decodeBitmap(src, (dec, info, s) -> {
                int w = info.getSize().getWidth(), h = info.getSize().getHeight();
                int scale = 1;
                while (Math.max(w, h) / scale > max) scale *= 2;
                if (scale > 1) dec.setTargetSize(w / scale, h / scale);
                dec.setAllocator(ImageDecoder.ALLOCATOR_SOFTWARE);
            });
        }
        BitmapFactory.Options o = new BitmapFactory.Options();
        o.inJustDecodeBounds = true;
        try (InputStream in = getContentResolver().openInputStream(uri)) { BitmapFactory.decodeStream(in, null, o); }
        int scale = 1;
        while (Math.max(o.outWidth, o.outHeight) / scale > max) scale *= 2;
        o = new BitmapFactory.Options();
        o.inSampleSize = scale;
        try (InputStream in = getContentResolver().openInputStream(uri)) { return BitmapFactory.decodeStream(in, null, o); }
    }

    private String displayName(Uri uri) {
        if ("file".equals(uri.getScheme())) return uri.getLastPathSegment() == null ? "photo" : uri.getLastPathSegment();
        try (Cursor c = getContentResolver().query(uri, new String[] { OpenableColumns.DISPLAY_NAME }, null, null, null)) {
            if (c != null && c.moveToFirst() && c.getString(0) != null) return c.getString(0);
        } catch (Exception ignored) {}
        return "photo";
    }

    @Override
    protected void onSaveInstanceState(Bundle out) {
        super.onSaveInstanceState(out);
        web.saveState(out);
    }

    @Override
    public void onBackPressed() {
        // let the page close its sheets/dialogs first
        web.evaluateJavascript("window.onBack && window.onBack()", v -> {
            if (!"true".equals(v)) MainActivity.super.onBackPressed();
        });
    }
}
