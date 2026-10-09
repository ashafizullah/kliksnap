import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Recording from "./Recording.svelte";

initI18n().then(() => mount(Recording, { target: document.body }));
