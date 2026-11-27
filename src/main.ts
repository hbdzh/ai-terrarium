import "./bootstrap";
import { createApp } from "vue";
import "./style.css";
import { initPlatformDetection } from "./utils/platform";
import { applyNativeInsetsFromBridge } from "./utils/native";

async function boot() {
  await initPlatformDetection();
  applyNativeInsetsFromBridge();
  const { default: App } = await import("./App.vue");
  createApp(App).mount("#app");
}

boot();
