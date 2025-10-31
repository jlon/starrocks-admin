import { Component, OnInit } from '@angular/core';
import { Router, NavigationEnd } from '@angular/router';
import { NbMenuService } from '@nebular/theme';
import { filter, map } from 'rxjs/operators';

import { MENU_ITEMS } from './pages-menu';
import { AuthService } from '../@core/data/auth.service';
import { TabService } from '../@core/services/tab.service';

@Component({
  selector: 'ngx-pages',
  styleUrls: ['pages.component.scss'],
  template: `
    <ngx-one-column-layout>
      <nb-menu [items]="menu" tag="menu" (itemClick)="onMenuClick($event)"></nb-menu>
      <router-outlet></router-outlet>
    </ngx-one-column-layout>
  `,
})
export class PagesComponent implements OnInit {
  menu = MENU_ITEMS;

  constructor(
    private menuService: NbMenuService,
    private authService: AuthService,
    private router: Router,
    private tabService: TabService
  ) {}

  ngOnInit() {
    // Listen to menu item clicks
    this.menuService.onItemClick()
      .pipe(
        filter(({ tag }) => tag === 'menu'),
        map(({ item }) => item)
      )
      .subscribe(item => {
        if (item.title === '退出登录') {
          this.authService.logout();
        }
      });

    // Listen to route changes and add tabs
    // Only handle route changes when navigation is triggered by menu or direct URL
    this.router.events
      .pipe(filter(event => event instanceof NavigationEnd))
      .subscribe((event: NavigationEnd) => {
        // Skip if this is triggered by tab switching
        // Check if the URL matches any existing active tab (使用规范化比较)
        const activeTab = this.tabService.getActiveTab();
        if (activeTab) {
          // 规范化比较 URL（处理编码问题）
          const normalizedEventUrl = this.normalizeUrl(event.url);
          const normalizedActiveTabUrl = this.normalizeUrl(activeTab.url);
          if (normalizedActiveTabUrl === normalizedEventUrl) {
            // This navigation is from tab switching, skip adding new tab
            return;
          }
        }
        this.handleRouteChange(event.url);
      });
  }

  /**
   * 处理路由变化，自动添加Tab
   */
  private handleRouteChange(url: string): void {
    // 跳过登录页面
    if (url.includes('/auth/')) {
      return;
    }

    // 对于带查询参数的 system 路由，优先使用 inferTitleFromUrl 获取更具体的标题
    let title: string | null = null;
    if (url.includes('/starrocks/system') && url.includes('?')) {
      title = this.inferTitleFromUrl(url);
    }

    // 如果没有通过 URL 推断获得标题，尝试查找菜单项
    if (!title) {
      const menuItem = this.findMenuItemByUrl(url);
      if (menuItem) {
        title = menuItem.title;
      }
    }

    // 如果还是没有标题，尝试从 URL 推断
    if (!title) {
      title = this.inferTitleFromUrl(url);
    }

    // 如果获得了标题，添加 Tab
    if (title) {
      const tabId = this.generateTabId(title);
      // 路由变化时不再触发导航（因为已经在目标路由了）
      this.tabService.addTab({
        id: tabId,
        title: title,
        url: url,
        closable: true,
        pinned: false
      }, false);
    }
  }

  /**
   * 规范化 URL，用于比较（处理编码问题）
   */
  private normalizeUrl(url: string): string {
    try {
      let decoded = url;
      try {
        decoded = decodeURIComponent(url);
      } catch (e) {
        decoded = url;
      }
      
      const [path, queryString] = decoded.split('?');
      
      let normalizedPath = path.replace(/\/+$/, '');
      if (!normalizedPath.startsWith('/')) {
        normalizedPath = '/' + normalizedPath;
      }
      
      if (queryString) {
        try {
          const params = new URLSearchParams(queryString);
          const sortedParams = Array.from(params.entries())
            .sort((a, b) => a[0].localeCompare(b[0]));
          const normalizedParams = new URLSearchParams(sortedParams);
          return normalizedPath + '?' + normalizedParams.toString();
        } catch (e) {
          return normalizedPath + '?' + queryString;
        }
      }
      
      return normalizedPath;
    } catch (e) {
      // 如果所有处理都失败，返回原始 URL
      return url;
    }
  }

  /**
   * 从URL推断标题
   */
  private inferTitleFromUrl(url: string): string | null {
    // 先解码 URL（处理可能的编码情况）
    try {
      url = decodeURIComponent(url);
    } catch (e) {
      // 如果解码失败，使用原始 URL
    }
    
    // 先分离路径和查询参数
    const [path, queryString] = url.split('?');
    const urlSegments = path.split('/').filter(segment => segment);
    
    // 处理StarRocks相关路由
    if (urlSegments.includes('starrocks')) {
      const lastSegment = urlSegments[urlSegments.length - 1];
      
      // 特殊处理 system 路由，从查询参数中提取功能名称
      if (lastSegment === 'system' && queryString) {
        // 处理可能的编码查询参数
        let decodedQueryString = queryString;
        try {
          decodedQueryString = decodeURIComponent(queryString);
        } catch (e) {
          // 如果解码失败，使用原始查询字符串
        }
        const params = new URLSearchParams(decodedQueryString);
        const functionName = params.get('function');
        if (functionName) {
          // 映射功能名称到中文标题（确保与 system-management.component.ts 中的定义一致）
          const functionTitleMap: { [key: string]: string } = {
            'backends': 'Backend节点信息',
            'frontends': 'Frontend节点信息',
            'brokers': 'Broker节点信息',
            'statistic': '统计信息',
            'dbs': '数据库信息',
            'tables': '表信息',
            'tablet_schema': 'Tablet Schema',
            'partitions': '分区信息',
            'transactions': '事务信息',
            'routine_loads': 'Routine Load任务',
            'stream_loads': 'Stream Load任务',
            'loads': 'Load任务',
            'load_error_hub': 'Load错误信息',
            'catalog': 'Catalog信息',
            'resources': '资源信息',
            'workload_groups': '工作负载组',
            'workload_sched_policy': '工作负载调度策略',
            'compactions': '压缩任务',
            'colocate_group': 'Colocate Group',
            'bdbje': 'BDBJE信息',
            'small_files': '小文件信息',
            'trash': '回收站',
            'jobs': '作业信息',
            'repositories': '仓库信息'
          };
          return functionTitleMap[functionName] || `功能卡片: ${functionName}`;
        }
      }
      
      // 映射URL段到中文标题
      const titleMap: { [key: string]: string } = {
        'dashboard': '集群列表',
        'overview': '集群概览',
        'frontends': 'Frontend 节点',
        'backends': 'Backend 节点',
        'execution': '实时查询',
        'profiles': 'Profiles',
        'audit-logs': '审计日志',
        'materialized-views': '物化视图',
        'system': '功能卡片',
        'sessions': '会话管理',
        'variables': '变量管理',
        'clusters': '集群管理',
        'new': '新建集群',
        'edit': '编辑集群'
      };
      
      return titleMap[lastSegment] || lastSegment;
    }
    
    return null;
  }

  /**
   * 根据URL查找菜单项
   */
  private findMenuItemByUrl(url: string): any {
    const findInMenu = (items: any[]): any => {
      for (const item of items) {
        if (item.link === url) {
          return item;
        }
        if (item.children) {
          const found = findInMenu(item.children);
          if (found) return found;
        }
      }
      return null;
    };

    return findInMenu(MENU_ITEMS);
  }

  /**
   * 生成Tab ID（基于标题，确保每个页面有唯一ID）
   */
  private generateTabId(title: string): string {
    // 使用标题生成固定ID，每个页面名称对应唯一ID
    return 'tab_' + title.replace(/[^a-zA-Z0-9\u4e00-\u9fa5]/g, '_');
  }

  onMenuClick(event: any) {
    if (event.item.title === '退出登录') {
      event.event.preventDefault();
      this.authService.logout();
    }
  }
}
