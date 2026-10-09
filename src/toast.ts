import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Toast from "./Toast.svelte";

initI18n().then(() => mount(Toast, { target: document.body }));
