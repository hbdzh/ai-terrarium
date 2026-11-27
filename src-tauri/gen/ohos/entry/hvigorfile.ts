import { hapTasks } from '@ohos/hvigor-ohos-plugin';
import { hvigor, HvigorPlugin, HvigorNode } from '@ohos/hvigor';
import { execFileSync } from 'child_process';
import { existsSync } from 'fs';
import { resolve } from 'path';

export default {
  system: hapTasks,  /* Built-in plugin of Hvigor. It cannot be modified. */
  plugins:[tauriPlugin()]         /* Custom plugin to extend the functionality of Hvigor. */
}

function tauriPlugin(): HvigorPlugin {
  return {
    pluginId: 'tauri',
    apply(node: HvigorNode) {
      const buildRustCode = () => {
        const properties = hvigor.getParameter().getProperties();
        const target = properties.target || "aarch64";
        const localToolsBin = resolve(__dirname, "../../../.ohos-tools/bin");
        const localCargoTauri = resolve(localToolsBin, "cargo-tauri");
        const prebuiltSo = resolve(__dirname, "libs/arm64-v8a/libai_terrarium_lib.so");

        // When invoked directly from DevEco Studio without `cargo-tauri ohos build/dev` WebSocket server,
        // skip `dev-eco-studio-script` if the prebuilt .so is already present.
        if (!process.env.TAURI_CLI_PORT && existsSync(prebuiltSo)) {
          return;
        }

        const cmd = existsSync(localCargoTauri) ? localCargoTauri : "cargo";
        const args = existsSync(localCargoTauri)
          ? ["tauri", "ohos", "dev-eco-studio-script", "--target", target.toString()]
          : ["tauri", "ohos", "dev-eco-studio-script", "--target", target.toString()];

        execFileSync(cmd, args, {
          cwd: resolve(__dirname, "../../../"),
          stdio: "inherit",
          env: {
            ...process.env,
            PATH: `${localToolsBin}:${process.env.PATH ?? ""}`,
          },
        });
      }

      node.getTaskByName('default@ConfigureCmake')!.afterRun(buildRustCode);
    }
  }
}
