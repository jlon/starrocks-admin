import { Injectable } from '@angular/core';
import { BehaviorSubject, Observable } from 'rxjs';
import { Router } from '@angular/router';

export interface TabItem {
  id: string;
  title: string;
  url: string;
  active: boolean;
  closable: boolean;
  pinned: boolean;
  icon?: string; // Optional icon for the tab
}

@Injectable({
  providedIn: 'root'
})
export class TabService {
  private readonly STORAGE_KEY = 'starrocks_admin_tabs';
  private tabsSubject = new BehaviorSubject<TabItem[]>([]);
  public tabs$ = this.tabsSubject.asObservable();

  constructor(private router: Router) {
    this.loadTabs();
    this.initializeDefaultTab();
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
      return url;
    }
  }

  /**
   * 添加新Tab，如果已存在则激活，不存在则创建
   * @param tab Tab信息
   * @param navigate 是否需要触发路由导航（默认true）
   */
  addTab(tab: Omit<TabItem, 'active'>, navigate: boolean = true): void {
    const currentTabs = this.tabsSubject.value;
    const normalizedNewUrl = this.normalizeUrl(tab.url);
    const existingTab = currentTabs.find(t => {
      const normalizedExistingUrl = this.normalizeUrl(t.url);
      return normalizedExistingUrl === normalizedNewUrl;
    });

    if (existingTab) {
      const updatedTabs = currentTabs.map(t => {
        if (t.id === existingTab.id) {
          return {
            ...t,
            title: tab.title, // 更新标题
            active: true
          };
        }
        return { ...t, active: false };
      });
      
      this.tabsSubject.next(updatedTabs);
      this.saveTabs();
      
      if (navigate && this.router.url !== tab.url) {
        const { path, queryParams } = this.parseUrl(tab.url);
        this.router.navigate(path, { queryParams });
      }
    } else {
      const newTab: TabItem = {
        ...tab,
        active: true
      };

      const updatedTabs = currentTabs.map(t => ({ ...t, active: false }));
      
      updatedTabs.push(newTab);
      
      this.tabsSubject.next(updatedTabs);
      this.saveTabs();
      
        if (navigate) {
          const { path, queryParams } = this.parseUrl(tab.url);
          this.router.navigate(path, { queryParams });
        }
    }
  }

  /**
   * 关闭指定Tab
   */
  closeTab(tabId: string): void {
    const currentTabs = this.tabsSubject.value;
    const tabToClose = currentTabs.find(t => t.id === tabId);
    
    if (!tabToClose || !tabToClose.closable) {
      return; // 不能关闭固定Tab
    }

    const updatedTabs = currentTabs.filter(t => t.id !== tabId);
    
    // 如果关闭的是当前激活Tab，需要激活其他Tab并刷新
    if (tabToClose.active && updatedTabs.length > 0) {
      const lastTab = updatedTabs[updatedTabs.length - 1];
      lastTab.active = true;
      
      // 关闭Tab时导航到新激活的Tab（这是需要刷新的场景）
      this.router.navigate([lastTab.url]);
    }
    
    this.tabsSubject.next(updatedTabs);
    this.saveTabs();
  }

  /**
   * 关闭左侧所有Tab（除固定外）
   */
  closeLeftTabs(tabId: string): void {
    const currentTabs = this.tabsSubject.value;
    const targetIndex = currentTabs.findIndex(t => t.id === tabId);
    
    if (targetIndex === -1) return;

    const targetTab = currentTabs[targetIndex];

    const filteredTabs = currentTabs.filter((tab, index) => {
      // 保留固定Tab或目标Tab及其右侧的Tab
      return tab.pinned || index >= targetIndex;
    });

    if (filteredTabs.length === 0) {
      return;
    }

    let activeTabId = filteredTabs.some(tab => tab.active) ? filteredTabs.find(tab => tab.active)!.id : null;
    const targetExists = filteredTabs.some(tab => tab.id === tabId);

    let updatedTabs = filteredTabs.map(tab => ({ ...tab }));

    if (targetExists) {
      updatedTabs = updatedTabs.map(tab => ({ ...tab, active: tab.id === tabId }));
      activeTabId = tabId;
    } else if (!activeTabId) {
      const fallback = updatedTabs[updatedTabs.length - 1];
      if (fallback) {
        updatedTabs = updatedTabs.map(tab => ({ ...tab, active: tab.id === fallback.id }));
        activeTabId = fallback.id;
      }
    }

    this.tabsSubject.next(updatedTabs);
    this.saveTabs();

    if (activeTabId) {
      const activeTab = updatedTabs.find(tab => tab.id === activeTabId);
      if (activeTab) {
        const { path, queryParams } = this.parseUrl(activeTab.url);
        this.router.navigate(path, { queryParams });
      }
    } else if (targetExists) {
      const { path, queryParams } = this.parseUrl(targetTab.url);
      this.router.navigate(path, { queryParams });
    }
  }

  /**
   * 关闭右侧所有Tab（除固定外）
   */
  closeRightTabs(tabId: string): void {
    const currentTabs = this.tabsSubject.value;
    const targetIndex = currentTabs.findIndex(t => t.id === tabId);
    
    if (targetIndex === -1) return;

    const filteredTabs = currentTabs.filter((tab, index) => {
      // 保留固定Tab或目标Tab及其左侧的Tab
      return tab.pinned || index <= targetIndex;
    });

    if (filteredTabs.length === 0) {
      return;
    }

    let activeTabId = filteredTabs.some(tab => tab.active) ? filteredTabs.find(tab => tab.active)!.id : null;
    const targetExists = filteredTabs.some(tab => tab.id === tabId);

    let updatedTabs = filteredTabs.map(tab => ({ ...tab }));

    if (targetExists) {
      updatedTabs = updatedTabs.map(tab => ({ ...tab, active: tab.id === tabId }));
      activeTabId = tabId;
    } else if (!activeTabId) {
      const fallback = updatedTabs[updatedTabs.length - 1];
      if (fallback) {
        updatedTabs = updatedTabs.map(tab => ({ ...tab, active: tab.id === fallback.id }));
        activeTabId = fallback.id;
      }
    }

    this.tabsSubject.next(updatedTabs);
    this.saveTabs();

    if (activeTabId) {
      const activeTab = updatedTabs.find(tab => tab.id === activeTabId);
      if (activeTab) {
        const { path, queryParams } = this.parseUrl(activeTab.url);
        this.router.navigate(path, { queryParams });
      }
    }
  }

  /**
   * 关闭其他所有Tab（除固定外）
   */
  closeOtherTabs(tabId: string): void {
    const currentTabs = this.tabsSubject.value;
    
    const filteredTabs = currentTabs.filter(tab => {
      // 保留固定Tab和目标Tab
      return tab.pinned || tab.id === tabId;
    });

    if (filteredTabs.length === 0) {
      return;
    }

    const updatedTabs = filteredTabs.map(tab => ({
      ...tab,
      active: tab.id === tabId,
    }));

    this.tabsSubject.next(updatedTabs);
    this.saveTabs();

    const activeTab = updatedTabs.find(tab => tab.id === tabId) || updatedTabs[updatedTabs.length - 1];
    if (activeTab) {
      const { path, queryParams } = this.parseUrl(activeTab.url);
      this.router.navigate(path, { queryParams });
    }
  }

  togglePin(tabId: string): void {
    const currentTabs = this.tabsSubject.value;
    const targetIndex = currentTabs.findIndex(tab => tab.id === tabId);

    if (targetIndex === -1) {
      return;
    }

    const targetTab = { ...currentTabs[targetIndex] };
    const toggledPinned = !targetTab.pinned;
    targetTab.pinned = toggledPinned;
    targetTab.closable = !toggledPinned;

    const updatedTabs = currentTabs.map((tab, index) => {
      if (index === targetIndex) {
        return targetTab;
      }
      return { ...tab };
    });

    const reordered = this.reorderTabs(updatedTabs);

    this.tabsSubject.next(reordered);
    this.saveTabs();
  }

  private reorderTabs(tabs: TabItem[]): TabItem[] {
    const pinnedTabs = tabs.filter(tab => tab.pinned);
    const otherTabs = tabs.filter(tab => !tab.pinned);

    return [...pinnedTabs, ...otherTabs];
  }

  /**
   * 解析 URL 字符串，返回路径和查询参数对象
   */
  private parseUrl(url: string): { path: string[], queryParams: any } {
    try {
      // 解码 URL
      const decoded = decodeURIComponent(url);
      const [path, queryString] = decoded.split('?');
      
      // 解析路径
      const pathSegments = path.split('/').filter(segment => segment);
      
      // 解析查询参数
      const queryParams: any = {};
      if (queryString) {
        const params = new URLSearchParams(queryString);
        params.forEach((value, key) => {
          queryParams[key] = value;
        });
      }
      
      return { path: ['/' + pathSegments.join('/')], queryParams };
    } catch (e) {
      // 如果解析失败，尝试作为路径直接使用
      return { path: [url], queryParams: {} };
    }
  }

  /**
   * 激活指定Tab
   * @param tabId Tab ID
   * @param navigate 是否需要触发路由导航（默认true）
   */
  activateTab(tabId: string, navigate: boolean = true): void {
    const currentTabs = this.tabsSubject.value;
    const targetTab = currentTabs.find(t => t.id === tabId);
    
    if (!targetTab) return;

    // Check if the target tab is already active
    const isAlreadyActive = targetTab.active;
    
    // Check if we're already on the target URL (使用规范化比较)
    const normalizedRouterUrl = this.normalizeUrl(this.router.url);
    const normalizedTargetUrl = this.normalizeUrl(targetTab.url);
    const isOnTargetUrl = normalizedRouterUrl === normalizedTargetUrl;

    // If already active and on target URL, do nothing
    if (isAlreadyActive && isOnTargetUrl) {
      return;
    }

    const updatedTabs = currentTabs.map(tab => ({
      ...tab,
      active: tab.id === tabId
    }));

    this.tabsSubject.next(updatedTabs);
    this.saveTabs();
    
    // Only navigate if needed and navigate flag is true
    // 正确解析 URL 并传递查询参数
    if (navigate && !isOnTargetUrl) {
      const { path, queryParams } = this.parseUrl(targetTab.url);
      this.router.navigate(path, { queryParams });
    }
  }


  /**
   * 保存Tab状态到localStorage
   */
  private saveTabs(): void {
    const tabs = this.tabsSubject.value;
    const tabsToSave = tabs.map(tab => ({
      id: tab.id,
      title: tab.title,
      url: tab.url,
      pinned: tab.pinned,
      closable: tab.closable,
      active: tab.active,  // Save active state
      icon: tab.icon  // Save icon
    }));
    
    localStorage.setItem(this.STORAGE_KEY, JSON.stringify(tabsToSave));
  }

  /**
   * 从localStorage恢复Tab状态
   */
  private loadTabs(): void {
    try {
      const savedTabs = localStorage.getItem(this.STORAGE_KEY);
      if (savedTabs) {
        const tabs: TabItem[] = JSON.parse(savedTabs).map((tab: any) => ({
          ...tab,
          active: tab.active || false,  // Restore active state from localStorage
          icon: tab.icon  // Restore icon
        }));
        this.tabsSubject.next(tabs);
      }
    } catch (error) {
      console.error('Failed to load tabs from localStorage:', error);
    }
  }

  /**
   * 初始化默认Tab（首页）
   */
  private initializeDefaultTab(): void {
    const currentTabs = this.tabsSubject.value;
    
    // 检查是否已有首页Tab
    const hasHomeTab = currentTabs.some(tab => tab.url === '/pages/starrocks/dashboard');
    
    if (!hasHomeTab) {
      const homeTab: TabItem = {
        id: 'home',
        title: '集群列表',
        url: '/pages/starrocks/dashboard',
        active: true,
        closable: false,
        pinned: true,
        icon: 'list-outline'  // Home tab icon
      };
      
      const updatedTabs = currentTabs.map(tab => ({ ...tab, active: false }));
      updatedTabs.unshift(homeTab);
      
      this.tabsSubject.next(updatedTabs);
      this.saveTabs();
    }
  }

  /**
   * 获取当前激活的Tab
   */
  getActiveTab(): TabItem | null {
    return this.tabsSubject.value.find(tab => tab.active) || null;
  }

  /**
   * 获取所有Tab
   */
  getTabs(): TabItem[] {
    return this.tabsSubject.value;
  }
}
