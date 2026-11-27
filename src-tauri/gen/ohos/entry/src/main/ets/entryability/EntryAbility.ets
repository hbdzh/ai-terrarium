import { RustAbility } from '@ohos-rs/ability'
import Want from '@ohos.app.ability.Want'
import { AbilityConstant } from '@kit.AbilityKit';
import { KeyboardAvoidMode } from '@kit.ArkUI';
import { deviceInfo } from '@kit.BasicServicesKit';
import { vibrator } from '@kit.SensorServiceKit';
import window from '@ohos.window';
import picker from '@ohos.file.picker';
import fileIo from '@ohos.file.fs';
import util from '@ohos.util';
import nativeLib from 'libai_terrarium_lib.so';

interface FileBridgeRequest {
  id: number;
  action: 'saveFromTemp' | 'openText';
  defaultName?: string;
  extensions?: string[];
  tempPath?: string;
}

interface UiBridgeRequest {
  action: 'setStatusBar' | 'haptic';
  lightIcons?: boolean;
  kind?: string;
}

export default class EntryAbility extends RustAbility {
  // change to dynamic library name
  public moduleName: string = "ai_terrarium_lib"
  public defaultPage: boolean = false;
  public mode: 'xcomponent' | 'webview' = 'webview'
  private mainWin: window.Window | null = null;
  private keyboardHeightVp: number = 0;

  private isDesktopDevice(): boolean {
    const t = (deviceInfo.deviceType || '').toLowerCase();
    return t === '2in1' || t === 'pc' || t === 'desktop';
  }

  private pushWindowInsets(win: window.Window): void {
    try {
      if (this.isDesktopDevice()) {
        nativeLib.updateOhosInsets(JSON.stringify({
          top: 0, right: 0, bottom: 0, left: 0, imeBottom: 0
        }));
        return;
      }
      const sysArea = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      const cutoutArea = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_CUTOUT);
      const navArea = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_NAVIGATION_INDICATOR);
      const kbdArea = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_KEYBOARD);

      const topPx = Math.max(sysArea.topRect.height, cutoutArea.topRect.height);
      const bottomPx = Math.max(sysArea.bottomRect.height, navArea.bottomRect.height);
      const leftPx = Math.max(sysArea.leftRect.width, cutoutArea.leftRect.width);
      const rightPx = Math.max(sysArea.rightRect.width, cutoutArea.rightRect.width);
      const imePx = kbdArea.bottomRect.height;

      const uiCtx = win.getUIContext();
      const top = Math.round(uiCtx.px2vp(topPx));
      const bottom = Math.round(uiCtx.px2vp(bottomPx));
      const left = Math.round(uiCtx.px2vp(leftPx));
      const right = Math.round(uiCtx.px2vp(rightPx));
      const imeBottom = Math.max(this.keyboardHeightVp, Math.round(uiCtx.px2vp(imePx)));

      nativeLib.updateOhosInsets(JSON.stringify({ top, right, bottom, left, imeBottom }));
    } catch {
      // Ignore if avoidArea is unavailable on current device
    }
  }

  private registerUiBridge(): void {
    nativeLib.registerOhosUiBridge((err: Error | null, payloadJson: string) => {
      if (err) return;
      try {
        const req = JSON.parse(payloadJson) as UiBridgeRequest;
        if (req.action === 'setStatusBar' && this.mainWin && !this.isDesktopDevice()) {
          const lightIcons = Boolean(req.lightIcons);
          this.mainWin.setWindowSystemBarProperties({
            statusBarColor: '#00000000',
            statusBarContentColor: lightIcons ? '#1D1D1F' : '#F4F4F5',
            navigationBarContentColor: lightIcons ? '#1D1D1F' : '#F4F4F5',
          }).catch(() => {});
        } else if (req.action === 'haptic') {
          const effectId =
            req.kind === 'confirm' || req.kind === 'medium'
              ? 'haptic.effect.hard'
              : 'haptic.effect.soft';
          vibrator.startVibration(
            { type: 'preset', effectId, count: 1 },
            { usage: 'touch' }
          ).catch(() => {});
        }
      } catch {
        // Ignore malformed UI bridge requests
      }
    });
  }

  private registerFileBridge(): void {
    nativeLib.registerOhosFileBridge((err: Error | null, payloadJson: string) => {
      if (err) {
        return;
      }
      this.handleFileBridgeRequest(payloadJson).catch(() => {});
    });
  }

  private async handleFileBridgeRequest(payloadJson: string): Promise<void> {
    let reqId = 0;
    try {
      const req = JSON.parse(payloadJson) as FileBridgeRequest;
      reqId = req.id;
      const documentPicker = new picker.DocumentViewPicker(this.context);

      if (req.action === 'saveFromTemp') {
        const saveOptions = new picker.DocumentSaveOptions();
        if (req.defaultName) {
          saveOptions.newFileNames = [req.defaultName];
        }
        if (req.extensions && req.extensions.length > 0) {
          saveOptions.fileSuffixChoices = req.extensions.map((ext) =>
            ext.startsWith('.') ? ext : `.${ext}`
          );
        }
        const uris = await documentPicker.save(saveOptions);
        if (!uris || uris.length === 0) {
          nativeLib.resolveOhosFileBridge(reqId, true, JSON.stringify({ cancelled: true }));
          return;
        }
        const destFile = fileIo.openSync(
          uris[0],
          fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE | fileIo.OpenMode.TRUNC
        );
        try {
          const srcFile = fileIo.openSync(req.tempPath!, fileIo.OpenMode.READ_ONLY);
          try {
            const stat = fileIo.statSync(srcFile.fd);
            const buf = new ArrayBuffer(stat.size);
            fileIo.readSync(srcFile.fd, buf);
            fileIo.writeSync(destFile.fd, buf);
          } finally {
            fileIo.closeSync(srcFile);
          }
        } finally {
          fileIo.closeSync(destFile);
        }
        nativeLib.resolveOhosFileBridge(reqId, true, JSON.stringify({ cancelled: false }));
        return;
      }

      if (req.action === 'openText') {
        const selectOptions = new picker.DocumentSelectOptions();
        selectOptions.maxSelectNumber = 1;
        if (req.extensions && req.extensions.length > 0) {
          selectOptions.fileSuffixFilters = req.extensions.map((ext) =>
            ext.startsWith('.') ? ext : `.${ext}`
          );
        }
        const uris = await documentPicker.select(selectOptions);
        if (!uris || uris.length === 0) {
          nativeLib.resolveOhosFileBridge(reqId, true, JSON.stringify({ cancelled: true }));
          return;
        }
        const uri = uris[0];
        const file = fileIo.openSync(uri, fileIo.OpenMode.READ_ONLY);
        let text = '';
        try {
          const stat = fileIo.statSync(file.fd);
          const buf = new ArrayBuffer(stat.size);
          fileIo.readSync(file.fd, buf);
          const decoder = util.TextDecoder.create('utf-8');
          text = decoder.decodeToString(new Uint8Array(buf));
        } finally {
          fileIo.closeSync(file);
        }
        const fileName = uri.split('/').pop() ?? 'imported';
        nativeLib.resolveOhosFileBridge(
          reqId,
          true,
          JSON.stringify({ cancelled: false, name: decodeURIComponent(fileName), content: text })
        );
        return;
      }

      nativeLib.resolveOhosFileBridge(reqId, false, `Unsupported file action: ${req.action}`);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (reqId > 0) {
        nativeLib.resolveOhosFileBridge(reqId, false, msg);
      }
    }
  }

  async onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): Promise<void> {
    if (this.context?.filesDir) {
      nativeLib.setOhosFilesDir(this.context.filesDir);
    }
    await super.onCreate(want, launchParam);
    this.registerFileBridge();
    this.registerUiBridge();
  }

  async onWindowStageCreate(windowStage: window.WindowStage): Promise<void> {
    const mainWin = windowStage.getMainWindowSync();
    this.mainWin = mainWin;
    if (!this.isDesktopDevice()) {
      await mainWin.setWindowLayoutFullScreen(true);
      this.pushWindowInsets(mainWin);
      mainWin.on('avoidAreaChange', () => {
        this.pushWindowInsets(mainWin);
      });
      mainWin.on('keyboardHeightChange', (heightPx: number) => {
        this.keyboardHeightVp = Math.round(mainWin.getUIContext().px2vp(heightPx));
        this.pushWindowInsets(mainWin);
      });
    } else {
      await mainWin.setWindowLayoutFullScreen(false);
    }
    await super.onWindowStageCreate(windowStage);
    await windowStage.loadContent('pages/Index');
    if (!this.isDesktopDevice()) {
      try {
        mainWin.getUIContext().setKeyboardAvoidMode(KeyboardAvoidMode.RESIZE);
      } catch {
        // Ignore if unsupported
      }
    }
  }
}
