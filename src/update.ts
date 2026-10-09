import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Update from "./Update.svelte";

initI18n().then(() => mount(Update, { target: document.body }));
