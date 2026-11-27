import "./bootstrap";
import { createApp } from "vue";
import "./style.css";
import App from "./App.vue";
import { initPlatformDetection, isMobileOS } from "./utils/platform";
import { applyNativeInsetsFromBridge } from "./utils/native";

applyNativeInsetsFromBridge();
createApp(App).mount("#app");

void initPlatformDetection().then(() => {
  document.documentElement.classList.toggle("mobile-os", isMobileOS);
});
