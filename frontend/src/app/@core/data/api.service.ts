import { Injectable } from '@angular/core';
import { HttpClient, HttpHeaders, HttpParams } from '@angular/common/http';
import { Observable } from 'rxjs';
import { timeout } from 'rxjs/operators';
import { environment } from '../../../environments/environment';

@Injectable({
  providedIn: 'root',
})
export class ApiService {
  private baseUrl = environment.apiUrl;

  constructor(private http: HttpClient) {}

  get<T>(path: string, params?: HttpParams | Record<string, any>): Observable<T> {
    let httpParams: HttpParams | undefined;
    if (params instanceof HttpParams) {
      httpParams = params;
    } else if (params && typeof params === 'object') {
      httpParams = new HttpParams({ fromObject: params as any });
    }
    return this.http.get<T>(`${this.baseUrl}${path}`, { params: httpParams });
  }

  post<T>(path: string, body: any = {}, customTimeout?: number): Observable<T> {
    const timeoutMs = customTimeout || 650000; // Default 650 seconds (10.8 minutes), slightly longer than Nginx timeout
    return this.http.post<T>(`${this.baseUrl}${path}`, body).pipe(
      timeout(timeoutMs)
    );
  }

  put<T>(path: string, body: any = {}): Observable<T> {
    return this.http.put<T>(`${this.baseUrl}${path}`, body);
  }

  delete<T>(path: string): Observable<T> {
    return this.http.delete<T>(`${this.baseUrl}${path}`);
  }
}

