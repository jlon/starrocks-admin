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
   * 添加新Tab，如果已存在则激活，不存在则创建
   * @param tab Tab信息
   * @param navigate 是否需要触发路由导航（默认true）
   */
  addTab(tab: Omit<TabItem, 'active'>, navigate: boolean = true): void {
    const currentTabs = this.tabsSubject.value;
    const existingTab = currentTabs.find(t => t.url === tab.url);

    if (existingTab) {
      // 如果Tab已存在，更新标题（如果不同）并激活
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
      
      // 只在需要时导航
      if (navigate && this.router.url !== tab.url) {
        this.router.navigate([tab.url]);
      }
    } else {
      // 如果Tab不存在，创建新Tab
      const newTab: TabItem = {
        ...tab,
        active: true
      };

      // 先取消所有Tab的激活状态
      const updatedTabs = currentTabs.map(t => ({ ...t, active: false }));
      
      // 添加新Tab
      updatedTabs.push(newTab);
      
      this.tabsSubject.next(updatedTabs);
      this.saveTabs();
      
      // 只在需要时导航到新Tab
      if (navigate) {
        this.router.navigate([tab.url]);
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

    const updatedTabs = currentTabs.filter((tab, index) => {
      // 保留固定Tab或目标Tab及其右侧的Tab
      return tab.pinned || index >= targetIndex;
    });

    this.tabsSubject.next(updatedTabs);
    this.saveTabs();
  }

  /**
   * 关闭右侧所有Tab（除固定外）
   */
  closeRightTabs(tabId: string): void {
    const currentTabs = this.tabsSubject.value;
    const targetIndex = currentTabs.findIndex(t => t.id === tabId);
    
    if (targetIndex === -1) return;

    const updatedTabs = currentTabs.filter((tab, index) => {
      // 保留固定Tab或目标Tab及其左侧的Tab
      return tab.pinned || index <= targetIndex;
    });

    this.tabsSubject.next(updatedTabs);
    this.saveTabs();
  }

  /**
   * 关闭其他所有Tab（除固定外）
   */
  closeOtherTabs(tabId: string): void {
    const currentTabs = this.tabsSubject.value;
    
    const updatedTabs = currentTabs.filter(tab => {
      // 保留固定Tab和目标Tab
      return tab.pinned || tab.id === tabId;
    });

    this.tabsSubject.next(updatedTabs);
    this.saveTabs();
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
    
    // Check if we're already on the target URL
    const isOnTargetUrl = this.router.url === targetTab.url;

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
    // This prevents unnecessary page reloads when switching between tabs
    if (navigate && !isOnTargetUrl) {
      this.router.navigate([targetTab.url]);
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
      active: tab.active  // Save active state
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
          active: tab.active || false  // Restore active state from localStorage
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
        pinned: true
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
