import { Injectable } from '@angular/core';
import { BehaviorSubject, Observable } from 'rxjs';
import { Router } from '@angular/router';
import { normalizeUrl } from '../utils/url-normalizer';
import { TabReuseService } from './tab-reuse.service';

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

  constructor(private router: Router, private tabReuseService: TabReuseService) {
    this.loadTabs();
    this.initializeDefaultTab();
  }

  /**
   * 添加新Tab，如果已存在则激活，不存在则创建
   * @param tab Tab信息
   * @param navigate 是否需要触发路由导航（默认true）
   */
  addTab(tab: Omit<TabItem, 'active'>, navigate: boolean = true): void {
    const currentTabs = this.tabsSubject.value;
    // Clean up any duplicated /pages/starrocks segments before processing
    const cleanedTab = {
      ...tab,
      url: tab.url.replace(/(\/pages\/starrocks)(?:\/pages\/starrocks)+/g, '$1')
    };
    const cleanedUrl = cleanedTab.url;
    
    // Check if tab already exists
    const existingTab = currentTabs.find(t => t.url === cleanedUrl);
    
    if (existingTab) {
      // Activate existing tab
      this.activateTab(existingTab.id, navigate);
      return;
    }
    
    // Create new tab
    const newTab: TabItem = {
      ...cleanedTab,
      active: true
    };
    
    // Deactivate all other tabs
    const updatedTabs = currentTabs.map(t => ({ ...t, active: false }));
    
    // Add new tab
    updatedTabs.push(newTab);
    
    // Update tabs
    this.tabsSubject.next(updatedTabs);
    this.saveTabs();
    
    // Navigate if needed
    if (navigate) {
      this.router.navigateByUrl(cleanedUrl);
    }
  }

  /**
   * 关闭指定Tab
   */
  closeTab(tabId: string): void {
    const currentTabs = this.tabsSubject.value;
    const tabToClose = currentTabs.find(tab => tab.id === tabId);
    
    if (!tabToClose || !tabToClose.closable) {
      return; // 不能关闭固定Tab
    }

    this.tabReuseService.markForRefresh(tabToClose.url);

    const updatedTabs = currentTabs.filter(t => t.id !== tabId);
    
    // 如果关闭的是当前激活Tab，需要激活其他Tab并刷新
    if (tabToClose.active && updatedTabs.length > 0) {
      const lastTab = updatedTabs[updatedTabs.length - 1];
      lastTab.active = true;
      
      // 关闭Tab时导航到新激活的Tab（这是需要刷新的场景）
      this.router.navigateByUrl(lastTab.url);
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

    const targetBefore = currentTabs[targetIndex];
    const wasActive = targetBefore?.active ?? false;

    const removedTabs = currentTabs.filter((tab, index) => !tab.pinned && index < targetIndex);

    const updatedTabs = currentTabs
      .filter((tab, index) => tab.pinned || index >= targetIndex)
      .map(tab => ({ ...tab, active: tab.id === tabId }));

    this.tabsSubject.next(updatedTabs);
    this.saveTabs();

    this.tabReuseService.markManyForRefresh(removedTabs.map(tab => tab.url));

    if (!wasActive) {
      const targetTab = updatedTabs.find(tab => tab.id === tabId);
      if (targetTab) {
        this.router.navigateByUrl(targetTab.url);
      }
    }
  }

  /**
   * 关闭右侧所有Tab（除固定外）
   */
  closeRightTabs(tabId: string): void {
    const currentTabs = this.tabsSubject.value;
    const targetIndex = currentTabs.findIndex(t => t.id === tabId);

    if (targetIndex === -1) return;

    const targetBefore = currentTabs[targetIndex];
    const wasActive = targetBefore?.active ?? false;

    const removedTabs = currentTabs.filter((tab, index) => !tab.pinned && index > targetIndex);

    const updatedTabs = currentTabs
      .filter((tab, index) => tab.pinned || index <= targetIndex)
      .map(tab => ({ ...tab, active: tab.id === tabId }));

    this.tabsSubject.next(updatedTabs);
    this.saveTabs();

    this.tabReuseService.markManyForRefresh(removedTabs.map(tab => tab.url));

    if (!wasActive) {
      const targetTab = updatedTabs.find(tab => tab.id === tabId);
      if (targetTab) {
        this.router.navigateByUrl(targetTab.url);
      }
    }
  }

  /**
   * 关闭其他所有Tab（除固定外）
   */
  closeOtherTabs(tabId: string): void {
    const currentTabs = this.tabsSubject.value;

    const removedTabs = currentTabs.filter(tab => !tab.pinned && tab.id !== tabId);
    const targetBefore = currentTabs.find(tab => tab.id === tabId);
    const wasActive = targetBefore?.active ?? false;

    const updatedTabs = currentTabs
      .filter(tab => tab.pinned || tab.id === tabId)
      .map(tab => ({ ...tab, active: tab.id === tabId }));

    this.tabsSubject.next(updatedTabs);
    this.saveTabs();

    this.tabReuseService.markManyForRefresh(removedTabs.map(tab => tab.url));

    if (!wasActive) {
      const activeTab = updatedTabs.find(tab => tab.id === tabId) || updatedTabs.find(tab => tab.active);
      if (activeTab) {
        this.router.navigateByUrl(activeTab.url);
      }
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
    const normalizedRouterUrl = normalizeUrl(this.router.url);
    const normalizedTargetUrl = normalizeUrl(targetTab.url);
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

  refreshTab(tabId: string): void {
    const currentTabs = this.tabsSubject.value;
    const targetTab = currentTabs.find(tab => tab.id === tabId);

    if (!targetTab) {
      return;
    }

    this.tabReuseService.markForRefresh(targetTab.url);

    if (targetTab.active) {
      this.router.navigateByUrl(targetTab.url);
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
        const tabs: TabItem[] = JSON.parse(savedTabs).map((tab: any) => {
          // Clean up any duplicated /pages/starrocks segments in saved URLs
          let cleanUrl = tab.url;
          if (cleanUrl && typeof cleanUrl === 'string') {
            cleanUrl = cleanUrl.replace(/(\/pages\/starrocks)(?:\/pages\/starrocks)+/g, '$1');
          }
          return {
            ...tab,
            url: cleanUrl,
            active: tab.active || false,
            icon: tab.icon
          };
        });
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
    
    // Check if already has home tab
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
