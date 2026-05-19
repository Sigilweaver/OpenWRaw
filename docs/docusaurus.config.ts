import { themes as prismThemes } from 'prism-react-renderer';
import type { Config } from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'OpenWRaw',
  tagline: 'Rust and Python reader for Waters MassLynx RAW data',
  favicon: 'img/favicon.ico',

  markdown: {
    mermaid: true,
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },
  themes: ['@docusaurus/theme-mermaid'],

  url: 'https://sigilweaver.app',
  baseUrl: '/openwraw/docs/',

  organizationName: 'Sigilweaver',
  projectName: 'OpenWRaw',

  onBrokenLinks: 'throw',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          routeBasePath: '/',
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/Sigilweaver/OpenWRaw/tree/main/docs/',
        },
        blog: false,
        sitemap: {
          changefreq: 'weekly',
          priority: 0.5,
          filename: 'sitemap.xml',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    metadata: [
      { name: 'keywords', content: 'OpenWRaw, Waters, MassLynx, RAW, mass spectrometry, Synapt, Xevo, Rust, Python' },
      { name: 'description', content: 'OpenWRaw is a Rust and Python reader for Waters MassLynx RAW data.' },
    ],
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: false,
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'OpenWRaw',
      logo: {
        alt: 'Sigilweaver logo',
        src: 'img/logo.svg',
        href: 'https://sigilweaver.app',
        target: '_self',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Docs',
        },
        {
          href: 'https://sigilweaver.app/openproteo/docs/',
          label: 'OpenProteo',
          position: 'left',
        },
        {
          href: 'https://docs.rs/openwraw',
          label: 'API (docs.rs)',
          position: 'right',
        },
        {
          href: 'https://sigilweaver.app',
          label: 'Website',
          position: 'right',
        },
        {
          href: 'https://github.com/Sigilweaver/OpenWRaw',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Project',
          items: [
            { label: 'GitHub', href: 'https://github.com/Sigilweaver/OpenWRaw' },
            { label: 'Issues', href: 'https://github.com/Sigilweaver/OpenWRaw/issues' },
            { label: 'crates.io', href: 'https://crates.io/crates/openwraw' },
            { label: 'docs.rs', href: 'https://docs.rs/openwraw' },
          ],
        },
        {
          title: 'Sigilweaver',
          items: [
            { label: 'Website', href: 'https://sigilweaver.app' },
            { label: 'Other projects', href: 'https://sigilweaver.app#projects' },
          ],
        },
        {
          title: 'Legal',
          items: [
            { label: 'Terms of Use', href: 'https://sigilweaver.app/terms' },
            { label: 'Privacy Policy', href: 'https://sigilweaver.app/privacy' },
          ],
        },
      ],
      copyright: `Copyright ${new Date().getFullYear()} Sigilweaver Holdings LLC. OpenWRaw is Apache-2.0 licensed. Documentation licensed under <a href="https://creativecommons.org/licenses/by-sa/4.0/" target="_blank" rel="noopener noreferrer">CC-BY-SA 4.0</a>.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'toml', 'bash'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
