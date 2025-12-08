import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { ApiService } from './api.service';

// LLM Provider model
export interface LLMProvider {
  id: number;
  name: string;
  display_name: string;
  api_base: string;
  model_name: string;
  api_key_masked?: string;  // Masked key for display
  is_active: boolean;
  enabled: boolean;
  max_tokens: number;
  temperature: number;
  timeout_seconds: number;
  priority: number;
  created_at: string;
  updated_at?: string;
}

// Create request
export interface CreateLLMProviderRequest {
  name: string;
  display_name: string;
  api_base: string;
  model_name: string;
  api_key: string;
  max_tokens?: number;
  temperature?: number;
  timeout_seconds?: number;
  priority?: number;
}

// Update request
export interface UpdateLLMProviderRequest {
  display_name?: string;
  api_base?: string;
  model_name?: string;
  api_key?: string;  // Only update if provided
  max_tokens?: number;
  temperature?: number;
  timeout_seconds?: number;
  priority?: number;
  enabled?: boolean;
}

// Test connection response
export interface TestConnectionResponse {
  success: boolean;
  message: string;
  latency_ms?: number;
}

// Usage statistics
export interface LLMUsageStats {
  date: string;
  provider_id: number;
  total_requests: number;
  successful_requests: number;
  failed_requests: number;
  total_input_tokens: number;
  total_output_tokens: number;
  avg_latency_ms: number;
  cache_hits: number;
  estimated_cost_usd: number;
}

@Injectable({
  providedIn: 'root',
})
export class LLMProviderService {
  private readonly basePath = '/llm/providers';

  constructor(private api: ApiService) {}

  // List all providers
  listProviders(): Observable<LLMProvider[]> {
    return this.api.get<LLMProvider[]>(this.basePath);
  }

  // Get single provider
  getProvider(id: number): Observable<LLMProvider> {
    return this.api.get<LLMProvider>(`${this.basePath}/${id}`);
  }

  // Get active provider
  getActiveProvider(): Observable<LLMProvider | null> {
    return this.api.get<LLMProvider | null>(`${this.basePath}/active`);
  }

  // Create provider
  createProvider(data: CreateLLMProviderRequest): Observable<LLMProvider> {
    return this.api.post<LLMProvider>(this.basePath, data);
  }

  // Update provider
  updateProvider(id: number, data: UpdateLLMProviderRequest): Observable<LLMProvider> {
    return this.api.put<LLMProvider>(`${this.basePath}/${id}`, data);
  }

  // Delete provider
  deleteProvider(id: number): Observable<void> {
    return this.api.delete<void>(`${this.basePath}/${id}`);
  }

  // Activate provider (deactivates all others)
  activateProvider(id: number): Observable<LLMProvider> {
    return this.api.post<LLMProvider>(`${this.basePath}/${id}/activate`, {});
  }

  // Deactivate provider
  deactivateProvider(id: number): Observable<LLMProvider> {
    return this.api.post<LLMProvider>(`${this.basePath}/${id}/deactivate`, {});
  }

  // Test connection
  testConnection(id: number): Observable<TestConnectionResponse> {
    return this.api.post<TestConnectionResponse>(`${this.basePath}/${id}/test`, {});
  }

  // Get usage statistics
  getUsageStats(providerId?: number, days?: number): Observable<LLMUsageStats[]> {
    let params: any = {};
    if (providerId) params.provider_id = providerId;
    if (days) params.days = days;
    return this.api.get<LLMUsageStats[]>('/llm/usage-stats', params);
  }
}
