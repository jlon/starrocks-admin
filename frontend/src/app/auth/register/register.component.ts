import { Component } from '@angular/core';
import { Router } from '@angular/router';
import { NbToastrService } from '@nebular/theme';
import { AuthService } from '../../@core/data/auth.service';
import { DiceBearService } from '../../@core/services/dicebear.service';

@Component({
  selector: 'ngx-register',
  templateUrl: './register.component.html',
  styleUrls: ['./register.component.scss']
})
export class RegisterComponent {
  submitted = false;
  user = {
    username: '',
    email: '',
    password: '',
    confirmPassword: '',
    avatar: ''
  };
  errors: string[] = [];
  messages: string[] = [];
  showMessages = false;
  selectedAvatarStyle = 'avataaars';

  // DiceBear头像选项
  availableAvatars: string[] = [];
  avatarStyles = this.diceBearService.avatarStyles;

  constructor(
    protected router: Router,
    private authService: AuthService,
    private toastrService: NbToastrService,
    private diceBearService: DiceBearService
  ) {
    this.generateAvatarOptions();
    // 随机选择一个头像
    if (this.availableAvatars.length > 0) {
      this.user.avatar = this.availableAvatars[Math.floor(Math.random() * this.availableAvatars.length)];
    }
  }

  selectAvatar(avatar: string) {
    this.user.avatar = avatar;
  }

  generateAvatarOptions() {
    this.availableAvatars = this.diceBearService.generateAvatarOptions(6, this.selectedAvatarStyle);
    // 如果当前没有选择头像，随机选择一个
    if (!this.user.avatar && this.availableAvatars.length > 0) {
      this.user.avatar = this.availableAvatars[Math.floor(Math.random() * this.availableAvatars.length)];
    }
  }

  register(): void {
    this.errors = [];
    this.messages = [];
    this.submitted = true;

    // Validation
    if (!this.user.username || !this.user.email || !this.user.password || !this.user.confirmPassword) {
      this.errors.push('All fields are required!');
      this.submitted = false;
      return;
    }

    if (this.user.password !== this.user.confirmPassword) {
      this.errors.push('Passwords do not match!');
      this.submitted = false;
      return;
    }

    if (this.user.password.length < 6) {
      this.errors.push('Password must be at least 6 characters long!');
      this.submitted = false;
      return;
    }

    const registerData = {
      username: this.user.username,
      email: this.user.email,
      password: this.user.password,
      avatar: this.user.avatar
    };

    this.authService.register(registerData).subscribe({
      next: (response) => {
        this.submitted = false;
        // Show single toast notification for registration success
        this.toastrService.success('Please login with your credentials', 'Registration Successful');
        // Navigate to login after short delay
        setTimeout(() => {
          this.router.navigate(['/auth/login']);
        }, 1500);
      },
      error: (error) => {
        this.submitted = false;
        // Show error in alert (form validation errors use alert, API errors use toast)
        const errorMessage = error.error?.message || 'Registration failed. Please try again.';
        this.errors = [errorMessage];
        this.showMessages = true;
        // Don't show toast for API errors since we already show alert
        // this.toastrService.danger(errorMessage, 'Registration Failed');
      }
    });
  }
}
