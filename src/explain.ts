import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Explain from "./Explain.svelte";

initI18n().then(() => mount(Explain, { target: document.body }));
