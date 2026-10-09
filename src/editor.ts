import { mount } from "svelte";
import "./app.css";
import { initI18n } from "./lib/i18n";
import Editor from "./Editor.svelte";

initI18n().then(() => mount(Editor, { target: document.body }));
