import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import History from "./History.svelte";

initI18n().then(() => mount(History, { target: document.body }));
