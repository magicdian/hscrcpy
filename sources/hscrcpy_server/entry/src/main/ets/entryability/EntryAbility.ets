import { AbilityConstant, ConfigurationConstant, UIAbility, Want } from '@kit.AbilityKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';
import nativeBridge from 'libentry.so';

const DOMAIN = 0x0000;
const TAG = 'hscrcpyShell';

export default class EntryAbility extends UIAbility {
  onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): void {
    try {
      this.context.getApplicationContext().setColorMode(ConfigurationConstant.ColorMode.COLOR_MODE_NOT_SET);
    } catch (err) {
      hilog.error(DOMAIN, TAG, 'Failed to set color mode. Cause: %{public}s', JSON.stringify(err));
    }
    try {
      const runtime = nativeBridge.startSessionListenerRuntime();
      hilog.info(
        DOMAIN,
        TAG,
        'Session listener bootstrap state=%{public}s port=%{public}d bound=%{public}s',
        runtime.state,
        runtime.sessionPort,
        runtime.listenerBound ? 'true' : 'false'
      );
      if (runtime.lastError.length > 0) {
        hilog.error(DOMAIN, TAG, 'Native session listener reported startup error: %{public}s', runtime.lastError);
      }
    } catch (err) {
      hilog.error(DOMAIN, TAG, 'Failed to bootstrap native session listener. Cause: %{public}s', JSON.stringify(err));
    }
    hilog.info(DOMAIN, TAG, 'Companion shell created');
  }

  onDestroy(): void {
    try {
      const runtime = nativeBridge.stopSessionListenerRuntime();
      hilog.info(
        DOMAIN,
        TAG,
        'Session listener stopped state=%{public}s port=%{public}d',
        runtime.state,
        runtime.sessionPort
      );
    } catch (err) {
      hilog.error(DOMAIN, TAG, 'Failed to stop native session listener. Cause: %{public}s', JSON.stringify(err));
    }
    hilog.info(DOMAIN, TAG, 'Companion shell destroyed');
  }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    hilog.info(DOMAIN, TAG, 'Loading companion shell page');

    windowStage.loadContent('pages/Index', (err) => {
      if (err.code) {
        hilog.error(DOMAIN, TAG, 'Failed to load the shell page. Cause: %{public}s', JSON.stringify(err));
        return;
      }
      hilog.info(DOMAIN, TAG, 'Shell page loaded');
    });
  }

  onWindowStageDestroy(): void {
    hilog.info(DOMAIN, TAG, 'Window stage destroyed');
  }

  onForeground(): void {
    hilog.info(DOMAIN, TAG, 'Companion shell moved to foreground');
  }

  onBackground(): void {
    hilog.info(DOMAIN, TAG, 'Companion shell moved to background');
  }
}
