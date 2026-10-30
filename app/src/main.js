import "@fontsource/ibm-plex-sans-thai/400.css";
import "@fontsource/ibm-plex-sans-thai/500.css";
import "@fontsource/ibm-plex-sans-thai/600.css";
import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";

mount(App, { target: document.getElementById("app") });
