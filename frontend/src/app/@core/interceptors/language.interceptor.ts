import { Injectable } from '@angular/core';
import {
  HttpRequest,
  HttpHandler,
  HttpEvent,
  HttpInterceptor,
} from '@angular/common/http';
import { Observable } from 'rxjs';
import { TranslateService } from '@ngx-translate/core';

/**
 * HTTP Interceptor that adds Accept-Language header to all requests.
 * This allows the backend to return localized messages.
 */
@Injectable()
export class LanguageInterceptor implements HttpInterceptor {
  constructor(private translate: TranslateService) {}

  intercept(request: HttpRequest<unknown>, next: HttpHandler): Observable<HttpEvent<unknown>> {
    const lang = this.translate.currentLang || this.translate.defaultLang || 'zh';
    
    const clonedRequest = request.clone({
      setHeaders: {
        'Accept-Language': lang,
      },
    });

    return next.handle(clonedRequest);
  }
}
