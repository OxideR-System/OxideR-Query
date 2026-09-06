import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

const config: Config = {
  title: 'OxideR-Query',
  tagline: 'Dựng câu lệnh SQL type-safe, đa dialect cho Rust',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: 'https://oxider-system.github.io',
  baseUrl: '/OxideR-Query/',

  organizationName: 'OxideR-System',
  projectName: 'OxideR-Query',

  onBrokenLinks: 'throw',
  onBrokenAnchors: 'throw',

  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
  },

  i18n: {
    defaultLocale: 'vi',
    locales: ['vi'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          // The guide lives in the repository's own `docs/` directory, so there
          // is one copy of it: readable as plain Markdown on GitHub, and
          // rendered by this site.
          path: '../docs',
          routeBasePath: 'docs',
          sidebarPath: './sidebars.ts',
          editUrl:
            'https://github.com/OxideR-System/OxideR-Query/tree/main/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'OxideR-Query',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'guideSidebar',
          position: 'left',
          label: 'Tài liệu',
        },
        {
          to: '/docs/api-cheatsheet',
          label: 'Tra cứu nhanh',
          position: 'left',
        },
        {
          href: 'https://github.com/OxideR-System/OxideR-Query',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Bắt đầu',
          items: [
            {label: 'Giới thiệu', to: '/docs/'},
            {label: 'Bắt đầu nhanh', to: '/docs/getting-started'},
            {label: 'Entity và cột', to: '/docs/entities-and-columns'},
          ],
        },
        {
          title: 'Tham khảo',
          items: [
            {label: 'Bộ toán tử', to: '/docs/operators'},
            {label: 'Dialect', to: '/docs/dialects'},
            {label: 'Tra cứu nhanh API', to: '/docs/api-cheatsheet'},
          ],
        },
        {
          title: 'Dự án',
          items: [
            {
              label: 'GitHub',
              href: 'https://github.com/OxideR-System/OxideR-Query',
            },
            {
              label: 'QueryDSL (nguồn cảm hứng)',
              href: 'https://github.com/OpenFeign/querydsl',
            },
          ],
        },
      ],
      copyright: `OxideR-Query, giấy phép MIT. Tài liệu xây dựng bằng Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'toml', 'sql', 'bash', 'json'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
