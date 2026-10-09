import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Preview from "./Preview.svelte";

initI18n().then(() => mount(Preview, { target: document.body }));
