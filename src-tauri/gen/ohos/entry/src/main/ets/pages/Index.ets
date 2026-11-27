import { DefaultXComponent } from '@ohos-rs/ability';
import nativeLib from 'libai_terrarium_lib.so';

@Entry
@Component
struct Index {
  onBackPress(): boolean {
    try {
      return nativeLib.onOhosBackPress();
    } catch {
      return false;
    }
  }

  build() {
    Row() {
      Column() {
        DefaultXComponent()
      }
      .width('100%')
    }
    .height('100%')
  }
}
