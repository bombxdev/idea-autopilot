# Idea Autopilot — reads a scanned PDF (or a picture) with OpenThai OCR Text and prints Markdown, one heading per page.
# Run by the app (docs.rs) through `uv run`, with the model folder as the first argument and the file as the second.
import os
import sys

MAX_PAGES = 60


def main():
    here, path = sys.argv[1], sys.argv[2]
    sys.path.insert(0, here)
    from runtime.line_engine import LineOCR

    ocr = LineOCR(here, threads=max(1, min(4, (os.cpu_count() or 2) // 2)))
    out = sys.stdout
    if path.lower().endswith(".pdf"):
        import pypdfium2 as pdfium

        pdf = pdfium.PdfDocument(path)
        n = len(pdf)
        for i in range(min(n, MAX_PAGES)):
            img = pdf[i].render(scale=200 / 72).to_pil()
            text = ocr.read(img)["text"].strip()
            out.write(f"## หน้า {i + 1}\n\n{text}\n\n")
            out.flush()
        if n > MAX_PAGES:
            out.write(f"(อ่านแค่ {MAX_PAGES} หน้าแรกจาก {n} หน้า)\n")
    else:
        out.write(ocr.read(path)["text"].strip() + "\n")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main()
