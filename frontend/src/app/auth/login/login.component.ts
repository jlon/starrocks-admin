import { Component, OnInit } from '@angular/core';
import { Router, ActivatedRoute } from '@angular/router';
import { NbToastrService } from '@nebular/theme';
import { AuthService } from '../../@core/data/auth.service';

@Component({
  selector: 'ngx-login',
  templateUrl: './login.component.html',
  styleUrls: ['./login.component.scss']
})
export class LoginComponent implements OnInit {
  submitted = false;
  user = {
    username: '',
    password: ''
  };
  rememberMe = false;
  errors: string[] = [];
  messages: string[] = [];
  showMessages = false;
  returnUrl: string;

  constructor(
    protected router: Router,
    private route: ActivatedRoute,
    private authService: AuthService,
    private toastrService: NbToastrService
  ) {}

  ngOnInit() {
    const rawReturnUrl = this.route.snapshot.queryParams['returnUrl'];
    this.returnUrl = this.authService.normalizeReturnUrl(rawReturnUrl);
    
    // Load saved username if remember me was checked
    const savedUsername = localStorage.getItem('remembered_username');
    if (savedUsername) {
      this.user.username = savedUsername;
      this.rememberMe = true;
    }
    
    // If already logged in, redirect to return URL using absolute navigation
    if (this.authService.isAuthenticated()) {
      this.router.navigateByUrl(this.returnUrl, { replaceUrl: true });
    }
  }

  login(): void {
    this.errors = [];
    this.messages = [];
    this.submitted = true;

    if (!this.user.username || !this.user.password) {
      this.errors.push('Username and password are required!');
      this.submitted = false;
      return;
    }

    this.authService.login(this.user).subscribe({
      next: (response) => {
        this.submitted = false;
        
        // Handle remember me functionality
        if (this.rememberMe) {
          localStorage.setItem('remembered_username', this.user.username);
        } else {
          localStorage.removeItem('remembered_username');
        }
        
        // Check if first login - redirect to password change page
        if (response.user.first_log) {
          // First login - redirect to user settings page without showing toast
          // The toast will be shown in the user-settings page
          setTimeout(() => {
            this.router.navigate(['/pages/user-settings'], { replaceUrl: true });
          }, 500);
        } else {
          // Normal login - show single toast notification for login success
          this.toastrService.success('Welcome back!', 'Login Successful');
          // Navigate to return URL using absolute navigation to prevent path duplication
          setTimeout(() => {
            this.router.navigateByUrl(this.returnUrl, { replaceUrl: true });
          }, 500);
        }
      },
      error: (error) => {
        this.submitted = false;
        let errorMessage = error.error?.message || 'Login failed. Please check your credentials.';
        let errorDetails = null;

        // 检查错误对象的类型
        if (typeof error === 'function') {
          try {
            const errorObj = error();
            if (errorObj) {
              // 检查是否是 HttpErrorResponse
              if (errorObj.error) {
                errorMessage = errorObj.error.message || errorMessage;
                errorDetails = errorObj.error.details;
              } else if (errorObj.message) {
                errorMessage = errorObj.message || errorMessage;
                errorDetails = errorObj.details;
              }
            }
          } catch (e) {
          }
        }
        // 方式1：直接从error.error获取
        else if (error.error) {
          errorMessage = error.error.message || errorMessage;
          errorDetails = error.error.details;
        }
        // 方式2：尝试从response获取
        else if (error.response) {
          errorMessage = error.response.message || errorMessage;
          errorDetails = error.response.details;
        }
        // 方式3：尝试解析错误消息
        else if (error.message) {
          try {
            const parsedError = JSON.parse(error.message);
            errorMessage = parsedError.message || errorMessage;
            errorDetails = parsedError.details;
          } catch (e) {
          }
        }
        // 方式4：检查是否有body属性
        else if (error.body) {
          errorMessage = error.body.message || errorMessage;
          errorDetails = error.body.details;
        }

        this.errors = [errorMessage];

        // 检查剩余尝试次数或锁定信息，在toast中显示信息
        if (errorDetails) {
          if (errorDetails.remaining_attempts !== undefined) {
            if (errorDetails.remaining_attempts > 0) {
              const toastMessage = `${errorMessage} (${errorDetails.remaining_attempts} attempts remaining)`;
              this.toastrService.danger(toastMessage, 'Login Failed');
            } else {
              // Account is locked
              if (errorDetails.locked_until) {
                const lockTime = new Date(errorDetails.locked_until);
                const lockTimeStr = lockTime.toLocaleString();
                const toastMessage = `${errorMessage} (Unlocks at ${lockTimeStr})`;
                this.toastrService.danger(toastMessage, 'Account Locked');
              }
            }
          }
        }

        this.showMessages = true;
      }
    });
  }
}
