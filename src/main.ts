import "./bootstrap";
import { createApp } from "vue";
import "./style.css";
import App from "./App.vue";
import { applyAndroidInsetsFromBridge } from "./utils/native";

applyAndroidInsetsFromBridge();
createApp(App).mount("#app");
