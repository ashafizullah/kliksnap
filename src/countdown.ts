import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Countdown from "./Countdown.svelte";

initI18n().then(() => mount(Countdown, { target: document.body }));
