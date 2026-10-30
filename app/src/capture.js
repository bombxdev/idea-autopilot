import "@fontsource/ibm-plex-sans-thai/400.css";
import "@fontsource/ibm-plex-sans-thai/500.css";
import "./app.css";
import { mount } from "svelte";
import Capture from "./Capture.svelte";

mount(Capture, { target: document.getElementById("app") });
