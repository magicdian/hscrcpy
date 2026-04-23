import { hilog } from '@kit.PerformanceAnalysisKit';

import { CompanionShellSection, loadCompanionShellModel } from '../bridge/CompanionShellBridge';

const DOMAIN = 0x0000;
const TAG = 'hscrcpyShell';

@Entry
@Component
struct Index {
  @State title: string = 'hscrcpy Harmony Companion';
  @State subtitle: string = 'Loading native foundation...';
  @State badge: string = 'SHELL';
  @State boundarySummary: string = 'ArkTS shell is waiting for native foundation data.';
  @State sections: CompanionShellSection[] = [];

  aboutToAppear(): void {
    try {
      const model = loadCompanionShellModel();
      this.title = model.title;
      this.subtitle = model.subtitle;
      this.badge = model.badge;
      this.boundarySummary = model.boundarySummary;
      this.sections = model.sections;
    } catch (err) {
      hilog.error(DOMAIN, TAG, 'Failed to load companion shell model. Cause: %{public}s', JSON.stringify(err));
      this.badge = 'BRIDGE ERROR';
      this.boundarySummary = 'Native bridge initialization failed. Check native contract wiring before session work.';
      this.sections = [
        {
          title: 'Native bridge status',
          lines: ['The companion shell could not read the native foundation descriptor.'],
        },
      ];
    }
  }

  build() {
    Scroll() {
      Column({ space: 16 }) {
        Column({ space: 10 }) {
          Text(this.badge)
            .fontSize($r('app.float.shell_badge_font_size'))
            .fontWeight(FontWeight.Medium)
            .fontColor($r('app.color.shell_badge_text'))
            .backgroundColor($r('app.color.shell_badge_background'))
            .padding({ left: 10, right: 10, top: 6, bottom: 6 })
            .borderRadius(999)

          Text(this.title)
            .fontSize($r('app.float.shell_title_font_size'))
            .fontWeight(FontWeight.Bold)
            .fontColor($r('app.color.shell_hero_text'))

          Text(this.subtitle)
            .fontSize($r('app.float.shell_subtitle_font_size'))
            .fontColor($r('app.color.shell_hero_text_secondary'))

          Text(this.boundarySummary)
            .fontSize($r('app.float.shell_body_font_size'))
            .fontColor($r('app.color.shell_hero_text_secondary'))
        }
        .width('100%')
        .padding(20)
        .backgroundColor($r('app.color.shell_hero_background'))
        .borderRadius(24)

        ForEach(this.sections, (section: CompanionShellSection) => {
          Column({ space: 12 }) {
            Text(section.title)
              .fontSize($r('app.float.shell_section_title_font_size'))
              .fontWeight(FontWeight.Medium)
              .fontColor($r('app.color.shell_text_primary'))

            ForEach(section.lines, (line: string) => {
              Text(`- ${line}`)
                .fontSize($r('app.float.shell_body_font_size'))
                .fontColor($r('app.color.shell_text_secondary'))
                .width('100%')
            }, (line: string) => line)
          }
          .width('100%')
          .padding(18)
          .backgroundColor($r('app.color.shell_card_background'))
          .borderRadius(20)
          .border({
            width: 1,
            color: $r('app.color.shell_card_border'),
          })
        }, (section: CompanionShellSection) => section.title)
      }
      .width('100%')
      .padding({ left: 16, right: 16, top: 24, bottom: 24 })
    }
    .width('100%')
    .height('100%')
    .backgroundColor($r('app.color.shell_page_background'))
  }
}
