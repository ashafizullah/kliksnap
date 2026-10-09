import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Overlay from "./Overlay.svelte";

initI18n().then(() => mount(Overlay, { target: document.body }));
