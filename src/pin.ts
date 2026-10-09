import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Pin from "./Pin.svelte";

initI18n().then(() => mount(Pin, { target: document.body }));
