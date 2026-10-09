import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Scroll from "./Scroll.svelte";

initI18n().then(() => mount(Scroll, { target: document.body }));
