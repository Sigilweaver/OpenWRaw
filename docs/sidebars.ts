import type { SidebarsConfig } from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'intro',
    'install',
    'quickstart',
    {
      type: 'category',
      label: 'Guide',
      collapsed: false,
      items: [
        'guide/reader',
        'guide/encodings',
        'guide/ims',
        'guide/chromatograms',
      ],
    },
    {
      type: 'category',
      label: 'Format Specification',
      link: { type: 'doc', id: 'format/overview' },
      items: [
        'format/overview',
        'format/header-txt',
        'format/functns-inf',
        'format/func-idx',
        'format/func-dat',
        'format/chroms-inf',
        'format/extern-inf',
        'format/func-sts',
        'format/chro-dat',
        'format/proc-files',
        'format/aux-files',
        'format/apex3d-bin',
      ],
    },
    'changelog',
    'license',
  ],
};

export default sidebars;
