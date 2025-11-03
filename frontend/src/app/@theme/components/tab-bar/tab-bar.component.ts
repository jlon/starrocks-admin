import { Component, OnInit, OnDestroy, ChangeDetectionStrategy, ChangeDetectorRef, HostListener } from '@angular/core';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { TabService, TabItem } from '../../../@core/services/tab.service';

type TabContextMenuAction = 'refresh' | 'close-left' | 'close-right' | 'close-others' | 'toggle-pin';

interface TabContextMenuItem {
  label: string;
  icon: string;
  action: TabContextMenuAction;
  disabled?: boolean;
}

@Component({
  selector: 'ngx-tab-bar',
  templateUrl: './tab-bar.component.html',
  styleUrls: ['./tab-bar.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class TabBarComponent implements OnInit, OnDestroy {
  tabs: TabItem[] = [];
  contextMenuVisible = false;
  contextMenuItems: TabContextMenuItem[] = [];
  contextMenuX = 0;
  contextMenuY = 0;
  private contextMenuTarget: TabItem | null = null;
  private destroy$ = new Subject<void>();

  constructor(
    private tabService: TabService,
    private cdr: ChangeDetectorRef
  ) {}

  ngOnInit(): void {
    this.tabService.tabs$
      .pipe(takeUntil(this.destroy$))
      .subscribe(tabs => {
        this.tabs = tabs;
        // Manually trigger change detection for OnPush strategy
        this.cdr.markForCheck();
      });
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  /**
   * 点击Tab激活
   */
  onTabClick(tab: TabItem): void {
    this.closeContextMenu();
    this.tabService.activateTab(tab.id);
  }

  /**
   * 关闭Tab
   */
  onCloseTab(event: Event, tabId: string): void {
    event.stopPropagation();
    this.closeContextMenu();
    this.tabService.closeTab(tabId);
  }

  onTabContextMenu(tab: TabItem, event: MouseEvent): void {
    event.preventDefault();
    event.stopPropagation();

    this.contextMenuTarget = tab;
    this.contextMenuItems = this.buildContextMenuItems(tab);

    const { x, y } = this.calculateMenuPosition(event, this.contextMenuItems.length);
    this.contextMenuX = x;
    this.contextMenuY = y;
    this.contextMenuVisible = true;
    this.cdr.markForCheck();
  }

  onContextMenuItemClick(item: TabContextMenuItem, event: MouseEvent): void {
    event.preventDefault();
    event.stopPropagation();

    if (item.disabled) {
      return;
    }

    this.handleContextMenuAction(item.action);
    this.closeContextMenu();
  }

  private handleContextMenuAction(action: TabContextMenuAction): void {
    if (!this.contextMenuTarget) {
      return;
    }

    switch (action) {
      case 'refresh':
        this.tabService.refreshTab(this.contextMenuTarget.id);
        break;
      case 'close-left':
        this.tabService.closeLeftTabs(this.contextMenuTarget.id);
        break;
      case 'close-right':
        this.tabService.closeRightTabs(this.contextMenuTarget.id);
        break;
      case 'close-others':
        this.tabService.closeOtherTabs(this.contextMenuTarget.id);
        break;
      case 'toggle-pin':
        this.tabService.togglePin(this.contextMenuTarget.id);
        break;
      default:
        break;
    }

    this.cdr.markForCheck();
  }

  private buildContextMenuItems(tab: TabItem): TabContextMenuItem[] {
    const targetIndex = this.tabs.findIndex(t => t.id === tab.id);

    const hasLeftClosable = this.tabs.some((item, index) => index < targetIndex && !item.pinned);
    const hasRightClosable = this.tabs.some((item, index) => index > targetIndex && !item.pinned);
    const hasOtherClosable = this.tabs.some((item) => item.id !== tab.id && !item.pinned);
    const isPinned = tab.pinned;

    return [
      {
        label: '刷新',
        icon: 'refresh-outline',
        action: 'refresh',
      },
      {
        label: '关闭左侧',
        icon: 'arrow-back-outline',
        action: 'close-left',
        disabled: !hasLeftClosable,
      },
      {
        label: '关闭右侧',
        icon: 'arrow-forward-outline',
        action: 'close-right',
        disabled: !hasRightClosable,
      },
      {
        label: '关闭其他',
        icon: 'minus-circle-outline',
        action: 'close-others',
        disabled: !hasOtherClosable,
      },
      {
        label: isPinned ? '取消固定' : '固定当前',
        icon: isPinned ? 'unlock-outline' : 'lock-outline',
        action: 'toggle-pin',
      },
    ];
  }

  private calculateMenuPosition(event: MouseEvent, itemCount: number): { x: number; y: number } {
    const menuWidth = 180;
    const menuHeight = itemCount * 40 + 12;
    let x = event.clientX;
    let y = event.clientY;

    if (x + menuWidth > window.innerWidth - 8) {
      x = Math.max(8, window.innerWidth - menuWidth - 8);
    }

    if (y + menuHeight > window.innerHeight - 8) {
      y = Math.max(8, window.innerHeight - menuHeight - 8);
    }

    return { x, y };
  }

  closeContextMenu(): void {
    if (!this.contextMenuVisible) {
      this.contextMenuTarget = null;
      return;
    }

    this.contextMenuVisible = false;
    this.contextMenuItems = [];
    this.contextMenuTarget = null;
    this.cdr.markForCheck();
  }

  @HostListener('document:click', ['$event'])
  onDocumentClick(event: MouseEvent): void {
    if (!this.contextMenuVisible) {
      return;
    }

    const target = event.target as HTMLElement | null;
    if (target && target.closest('.tab-context-menu')) {
      return;
    }

    this.closeContextMenu();
  }

  @HostListener('window:scroll')
  onWindowScroll(): void {
    this.closeContextMenu();
  }

  @HostListener('window:resize')
  onWindowResize(): void {
    this.closeContextMenu();
  }

  /**
   * TrackBy函数优化渲染性能
   */
  trackByTabId(index: number, tab: TabItem): string {
    return tab.id;
  }
}
