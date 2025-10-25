import { NbMenuItem } from '@nebular/theme';

export const MENU_ITEMS: NbMenuItem[] = [
  {
    title: '集群列表',
    icon: 'list-outline',
    link: '/pages/starrocks/dashboard',
    home: true,
  },
  {
    title: '集群概览',
    icon: 'activity-outline',
    link: '/pages/starrocks/overview',
  },
  {
    title: '集群管理',
    icon: 'layers-outline',
    link: '/pages/starrocks/clusters',
  },
  {
    title: '节点管理',
    icon: 'hard-drive-outline',
    children: [
      {
        title: 'Frontend 节点',
        link: '/pages/starrocks/frontends',
      },
      {
        title: 'Backend 节点',
        link: '/pages/starrocks/backends',
      },
    ],
  },
  {
    title: '查询管理',
    icon: 'code-outline',
    link: '/pages/starrocks/queries',
  },
  {
    title: '物化视图',
    icon: 'cube-outline',
    link: '/pages/starrocks/materialized-views',
  },
  {
    title: '功能卡片',
    icon: 'grid-outline',
    link: '/pages/starrocks/system',
  },
  {
    title: '会话管理',
    icon: 'person-outline',
    link: '/pages/starrocks/sessions',
  },
  {
    title: '变量管理',
    icon: 'settings-2-outline',
    link: '/pages/starrocks/variables',
  },
  {
    title: '监控指标',
    icon: 'bar-chart-outline',
    link: '/pages/starrocks/monitor',
  },
];
