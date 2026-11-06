# Page brief

- Type and state: Desktop app, live. Open-source dev tool rules also apply (GitHub link, license, privacy and price stated plainly). Supporting pages: privacy (it's an app). No separate download page: the CTA goes to GitHub's latest-release URL, which already resolves the latest build with no script. No changelog page: there are no dated entries in the project, so GitHub Releases does this job.
- Audience: Dart and Flutter developers on a Mac who look after one or more projects. They know pub.dev and `dart pub outdated`. They don't know Spotter.
- The page's one job: get a Mac-using Dart or Flutter developer to download Spotter.
- Primary CTA: "Download for Mac" to https://github.com/spotter-demo/spotter/releases/latest
- Secondary action: "View on GitHub" to https://github.com/spotter-demo/spotter
- Search phrases: flutter dependency update checker mac; dart package new release notification; pub.dev update notifier; check outdated flutter packages; flutter menu bar app
- Objections: (1) "I can just run `dart pub outdated`"; (2) "What does it send, and to whom?"; (3) "What does it cost, and can I read the code?"; (4) "Will it run on my Mac?"
- Analytics and cookies: none. The site loads no JavaScript.

Audience and search phrases are inferred from the README. No owner was available to confirm them.

## Claims

| Claim on the page | Evidence |
| --- | --- |
| Free, open-source, MIT license | README.md, read 2026-11-06. The repository has no LICENSE file. |
| macOS menu bar app that watches Dart and Flutter projects and reports new dependency releases | README.md, read 2026-11-06 |
| Spotter checks pub.dev | Inferred from README.md: its job is reporting new releases, and pub.dev is its only network destination. Read 2026-11-06. |
| Download is a DMG on GitHub Releases | README.md, read 2026-11-06. Not confirmed: https://github.com/spotter-demo/spotter and /releases/latest both returned 404 on 2026-11-06. |
| Written in Swift (llms.txt only) | README.md, read 2026-11-06 |
| macOS 14 or later | README.md, read 2026-11-06 |
| No network calls except to pub.dev; no accounts; no analytics | README.md, read 2026-11-06. App source is not in this repository, so this can't be measured here. |
| `dart pub outdated` shows current, upgradable, resolvable and latest versions | https://dart.dev/tools/pub/cmd/pub-outdated, read 2026-11-06 |
| This site loads no JavaScript, sets no cookies and has no forms | site/ source, 2026-11-06 |

## Left off the page until the owner supplies them

- A real screenshot. assets/menu.png was measured on 2026-11-06: 1600x1000, every pixel #304060, so it shows no interface. The hero has no image until a real capture exists.
- "Notarized" and "version 1.2.0": both are in README.md, but no release could be opened on 2026-11-06.

## Choices made without the owner

- Kept "free", "open source", "MIT", the DMG download and the pub.dev network claim on the README's word, because removing them leaves no page. Review findings B1 and B3 stay open until the repository is public and the owner confirms the network claim.
