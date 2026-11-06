# A small theme for one page

For a project with no theme and no app to port one from. One CSS file, written by hand. It's enough
for a page or two, and it needs no design records.

Use the role names below. They match the `ultrahand:theme` output, so a real theme can replace this
file later without touching the page. Choose the values fresh for each project, or every page ends
up looking the same.

```css
:root {
  color-scheme: light dark;

  --color-surface: light-dark(<light>, <dark>);
  --color-on-surface: light-dark(<light>, <dark>);
  --color-on-surface-variant: light-dark(<light>, <dark>);
  --color-primary: light-dark(<light>, <dark>);
  --color-on-primary: light-dark(<light>, <dark>);
  --color-outline: light-dark(<light>, <dark>);
  --color-focus-ring: light-dark(<light>, <dark>);

  --font-body: <stack>;
  --font-display: <stack, or var(--font-body)>;

  --space-gap-related: 8px;
  --space-gap-group: 24px;
  --space-gap-section: 80px;
  --space-inset: 16px;

  --radius-control: <one value>;
  --focus-width: 2px;
}
```

## Choosing values

- **Colour:** one accent. Take it from something the product already has: its icon, a screenshot,
  its terminal output. Only `--color-primary` carries it. Keep the surfaces quiet.
- **Fonts:** system fonts are fine. Two WOFF2 files at most, per the budget in `seo.md`.
- **Tells:** check the list in `message.md`. Purple gradients and permanent dark mode start here.

## What has to hold

These apply however small the page is. The reviewer's accessibility pass tests the measurable ones.

- Light and dark values for every colour, chosen rather than inverted.
- Contrast of at least 4.5:1 for text and 3:1 for controls, measured in both schemes.
- Body text at least 1rem with a line height of 1.5. Sizes in `rem`, about three of them.
- Spacing only from 4, 8, 12, 16, 24, 32, 40, 48, 64, 80 and 96.
- A visible focus ring, using `--focus-width` and `--color-focus-ring`.
- The page uses `var(...)` for all of these, and no raw colours or one-off sizes.
