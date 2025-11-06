import { Component, OnInit } from '@angular/core';
import { Router } from '@angular/router';
import { NbToastrService } from '@nebular/theme';
import { AuthService, User } from '../../@core/data/auth.service';
import { ApiService } from '../../@core/data/api.service';
import { DiceBearService } from '../../@core/services/dicebear.service';

@Component({
  selector: 'ngx-user-settings',
  templateUrl: './user-settings.component.html',
  styleUrls: ['./user-settings.component.scss']
})
export class UserSettingsComponent implements OnInit {
  loading = false;
  submitted = false;
  currentUser: User | null = null;
  
  userForm = {
    username: '',
    email: '',
    avatar: '',
    currentPassword: '',
    newPassword: '',
    confirmPassword: ''
  };

  errors: string[] = [];
  showPasswordFields = false;
  showAvatarSelection = false;
  selectedAvatarStyle = 'avataaars';

  // DiceBear头像选项
  availableAvatars: string[] = [];
  avatarStyles = this.diceBearService.avatarStyles;

  constructor(
    private authService: AuthService,
    private apiService: ApiService,
    private toastrService: NbToastrService,
    private router: Router,
    private diceBearService: DiceBearService
  ) {}

  ngOnInit() {
    this.loadUserInfo();
  }

  loadUserInfo() {
    this.loading = true;
    this.authService.getMe().subscribe({
      next: (user: any) => {
        this.currentUser = user;
        this.userForm.username = user.username;
        this.userForm.email = user.email || '';
        this.userForm.avatar = user.avatar || this.availableAvatars[0];
        this.loading = false;
      },
      error: (error) => {
        this.toastrService.danger('Failed to load user information', 'Error');
        this.loading = false;
      }
    });
  }

  selectAvatar(avatar: string) {
    this.userForm.avatar = avatar;
  }

  generateAvatarOptions() {
    this.availableAvatars = this.diceBearService.generateAvatarOptions(6, this.selectedAvatarStyle);
  }

  onAvatarStyleChange() {
    this.generateAvatarOptions();
  }

  toggleAvatarSelection() {
    this.showAvatarSelection = !this.showAvatarSelection;
    if (this.showAvatarSelection && this.availableAvatars.length === 0) {
      this.generateAvatarOptions();
    }
  }

  togglePasswordFields() {
    this.showPasswordFields = !this.showPasswordFields;
    if (!this.showPasswordFields) {
      this.userForm.currentPassword = '';
      this.userForm.newPassword = '';
      this.userForm.confirmPassword = '';
    }
  }

  onSubmit() {
    this.errors = [];
    this.submitted = true;

    // Validation
    if (!this.userForm.username || !this.userForm.email) {
      this.errors.push('用户名和邮箱不能为空');
      this.submitted = false;
      return;
    }

    // Validate email format
    const emailPattern = /^[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,4}$/;
    if (!emailPattern.test(this.userForm.email)) {
      this.errors.push('请输入有效的邮箱地址');
      this.submitted = false;
      return;
    }

    // If changing password, validate password fields
    if (this.showPasswordFields) {
      if (!this.userForm.currentPassword) {
        this.errors.push('请输入当前密码');
        this.submitted = false;
        return;
      }

      if (!this.userForm.newPassword) {
        this.errors.push('请输入新密码');
        this.submitted = false;
        return;
      }

      if (this.userForm.newPassword.length < 6) {
        this.errors.push('新密码至少需要6个字符');
        this.submitted = false;
        return;
      }

      if (this.userForm.newPassword !== this.userForm.confirmPassword) {
        this.errors.push('两次输入的密码不一致');
        this.submitted = false;
        return;
      }
    }

    // Prepare update data
    const updateData: any = {
      username: this.userForm.username,
      email: this.userForm.email,
      avatar: this.userForm.avatar
    };

    if (this.showPasswordFields && this.userForm.newPassword) {
      updateData.current_password = this.userForm.currentPassword;
      updateData.new_password = this.userForm.newPassword;
    }

    // Check if password is being changed
    const isChangingPassword = this.showPasswordFields && this.userForm.newPassword;

    // Call API
    this.apiService.put(`/auth/me`, updateData).subscribe({
      next: (response: any) => {
        this.submitted = false;
        
        // If password was changed, logout and redirect to login
        if (isChangingPassword) {
          this.toastrService.success('密码修改成功，请重新登录', '成功');
          setTimeout(() => {
            // Clear auth data and redirect to login
            localStorage.removeItem('jwt_token');
            localStorage.removeItem('current_user');
            this.router.navigate(['/auth/login']);
          }, 1500);
        } else {
          // Just show success message
          this.toastrService.success('用户信息更新成功', '成功');
          
          // Fetch latest user info from database to ensure we have the latest data
          this.authService.getMe().subscribe({
            next: (user) => {
              // Update current user in AuthService to trigger header update
              this.authService.updateCurrentUser(user);
              
              // Update form with latest data
              this.userForm.username = user.username;
              this.userForm.email = user.email || '';
              this.userForm.avatar = (user as any).avatar || this.availableAvatars[0];
              
              // Reset password fields
              this.showPasswordFields = false;
              this.userForm.currentPassword = '';
              this.userForm.newPassword = '';
              this.userForm.confirmPassword = '';
              
              // Redirect to cluster list page after successful update
              setTimeout(() => {
                this.router.navigate(['/pages/starrocks/clusters']);
              }, 1500);
            },
            error: (error) => {
              console.error('Failed to reload user info:', error);
              // Even if reload fails, reset password fields
              this.showPasswordFields = false;
              this.userForm.currentPassword = '';
              this.userForm.newPassword = '';
              this.userForm.confirmPassword = '';
              
              // Still redirect to cluster list page even if reload fails
              setTimeout(() => {
                this.router.navigate(['/pages/starrocks/clusters']);
              }, 1500);
            }
          });
        }
      },
      error: (error) => {
        this.submitted = false;
        // Show error in alert (form validation errors use alert, API errors use alert too for consistency)
        const errorMessage = error.error?.message || '更新失败，请重试';
        this.errors = [errorMessage];
        // Don't show toast for API errors since we already show alert
        // this.toastrService.danger(errorMessage, '错误');
      }
    });
  }

  onCancel() {
    this.router.navigate(['/pages/starrocks/dashboard']);
  }
}

