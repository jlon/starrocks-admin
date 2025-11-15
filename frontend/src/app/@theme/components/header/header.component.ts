import { Component, OnDestroy, OnInit } from '@angular/core';
import { Router } from '@angular/router';
import { NbMediaBreakpointsService, NbMenuService, NbSidebarService, NbThemeService, NbToastrService } from '@nebular/theme';
import { TranslateService } from '@ngx-translate/core';

import { LayoutService } from '../../../@core/utils';
import { AuthService } from '../../../@core/data/auth.service';
import { map, takeUntil, filter } from 'rxjs/operators';
import { Subject } from 'rxjs';

@Component({
  selector: 'ngx-header',
  styleUrls: ['./header.component.scss'],
  templateUrl: './header.component.html',
})
export class HeaderComponent implements OnInit, OnDestroy {

  private destroy$: Subject<void> = new Subject<void>();
  userPictureOnly: boolean = false;
  user: any;

  themes = [
    {
      value: 'default',
      name: 'theme.default',
    },
    {
      value: 'dark',
      name: 'theme.dark',
    },
    {
      value: 'cosmic',
      name: 'theme.cosmic',
    },
    {
      value: 'corporate',
      name: 'theme.corporate',
    },
  ];

  currentTheme = 'default';
  currentThemeLabel = 'theme.default';

  languages = [
    {
      value: 'zh',
      name: '中文',
    },
    {
      value: 'en',
      name: 'English',
    },
  ];

  currentLanguage = 'zh';

  userMenu = [
    { title: this.translate.instant('header.user_settings'), icon: 'settings-outline', data: { id: 'settings' } },
    { title: this.translate.instant('header.logout'), icon: 'log-out-outline', data: { id: 'logout' } },
  ];

  languageMenu = [
    { title: this.translate.instant('header.chinese'), data: { lang: 'zh' } },
    { title: this.translate.instant('header.english'), data: { lang: 'en' } },
  ];

  constructor(
    private sidebarService: NbSidebarService,
    private menuService: NbMenuService,
    private themeService: NbThemeService,
    private authService: AuthService,
    private layoutService: LayoutService,
    private breakpointService: NbMediaBreakpointsService,
    private router: Router,
    private toastr: NbToastrService,
    private translate: TranslateService,
  ) {
  }

  ngOnInit() {
    this.currentTheme = this.themeService.currentTheme;
    this.updateThemeLabel();

    // Initialize language from localStorage or use default
    const savedLanguage = localStorage.getItem('language') || 'zh';
    this.currentLanguage = savedLanguage;
    
    // Set default language synchronously first
    this.translate.setDefaultLang('zh');
    this.translate.use(savedLanguage);

    // Update theme label when language changes
    this.translate.onLangChange.pipe(takeUntil(this.destroy$)).subscribe(() => {
      this.updateThemeLabel();
    });

    // Get current user info
    this.authService.currentUser
      .pipe(takeUntil(this.destroy$))
      .subscribe(currentUser => {
        if (currentUser) {
          this.user = {
            name: currentUser.username || 'admin',
            picture: currentUser.avatar || 'assets/images/nick.png', // 使用用户的头像，如果没有则使用默认头像
          };
        } else {
          // 如果未登录，设置默认用户
          this.user = {
            name: 'admin',
            picture: 'assets/images/nick.png',
          };
        }
      });

    // Handle user menu clicks
    this.menuService.onItemClick()
      .pipe(
        filter(({ tag }) => tag === 'user-context-menu'),
        map(({ item }) => item),
        takeUntil(this.destroy$),
      )
      .subscribe(item => {
        if (item.data) {
          switch (item.data.id) {
            case 'settings':
              this.router.navigate(['/pages/user-settings']);
              break;
            case 'logout':
              this.logout();
              break;
          }
        }
      });

    // Handle language menu clicks
    this.menuService.onItemClick()
      .pipe(
        filter(({ tag }) => tag === 'language-menu'),
        map(({ item }) => item),
        takeUntil(this.destroy$),
      )
      .subscribe(item => {
        if (item.data && item.data.lang) {
          this.changeLanguage(item.data.lang);
        }
      });

    const { xl } = this.breakpointService.getBreakpointsMap();
    this.themeService.onMediaQueryChange()
      .pipe(
        map(([, currentBreakpoint]) => currentBreakpoint.width < xl),
        takeUntil(this.destroy$),
      )
      .subscribe((isLessThanXl: boolean) => this.userPictureOnly = isLessThanXl);

    this.themeService.onThemeChange()
      .pipe(
        map(({ name }) => name),
        takeUntil(this.destroy$),
      )
      .subscribe(themeName => this.currentTheme = themeName);
  }

  ngOnDestroy() {
    this.destroy$.next();
    this.destroy$.complete();
  }

  changeTheme(themeName: string) {
    this.themeService.changeTheme(themeName);
    this.currentTheme = themeName;
    this.updateThemeLabel();
  }

  private updateThemeLabel(): void {
    const theme = this.themes.find(t => t.value === this.currentTheme);
    if (theme) {
      this.currentThemeLabel = theme.name;
    }
  }

  changeLanguage(language: string) {
    this.currentLanguage = language;
    this.translate.use(language);
    localStorage.setItem('language', language);
  }

  toggleSidebar(): boolean {
    this.sidebarService.toggle(true, 'menu-sidebar');
    this.layoutService.changeLayoutSize();

    return false;
  }

  navigateHome() {
    this.menuService.navigateHome();
    return false;
  }

  logout() {
    this.toastr.success(this.translate.instant('header.logout_success'), this.translate.instant('header.hint'));
    setTimeout(() => {
      this.authService.logout();
    }, 500);
  }
}
