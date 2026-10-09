import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Settings from "./Settings.svelte";

initI18n().then(() => mount(Settings, { target: document.body }));
