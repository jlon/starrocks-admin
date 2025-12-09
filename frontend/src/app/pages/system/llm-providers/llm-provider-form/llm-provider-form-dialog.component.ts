import { Component, Input, OnInit } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { NbDialogRef } from '@nebular/theme';

import { LLMProvider } from '../../../../@core/data/llm-provider.service';

export type LLMProviderFormMode = 'create' | 'edit';

export interface LLMProviderFormDialogResult {
  name: string;
  display_name: string;
  api_base: string;
  model_name: string;
  api_key?: string;
  max_tokens: number;
  temperature: number;
  timeout_seconds: number;
  priority: number;
}

@Component({
  selector: 'ngx-llm-provider-form-dialog',
  templateUrl: './llm-provider-form-dialog.component.html',
  styleUrls: ['./llm-provider-form-dialog.component.scss'],
})
export class LLMProviderFormDialogComponent implements OnInit {
  @Input() mode: LLMProviderFormMode = 'create';
  @Input() provider?: LLMProvider;

  form: FormGroup;
  showApiKey = false;

  // Common presets for quick selection
  presets = [
    { name: 'openai', display: 'OpenAI', api_base: 'https://api.openai.com/v1', model: 'gpt-4o' },
    { name: 'deepseek', display: 'DeepSeek', api_base: 'https://api.deepseek.com/v1', model: 'deepseek-chat' },
    { name: 'qwen', display: '通义千问', api_base: 'https://dashscope.aliyuncs.com/compatible-mode/v1', model: 'qwen-plus' },
    { name: 'openrouter', display: 'OpenRouter', api_base: 'https://openrouter.ai/api/v1', model: 'openai/gpt-4o' },
    { name: 'custom', display: '自定义', api_base: '', model: '' },
  ];

  constructor(
    private dialogRef: NbDialogRef<LLMProviderFormDialogComponent>,
    private fb: FormBuilder,
  ) {
    this.form = this.fb.group({
      name: ['', [Validators.required, Validators.maxLength(50), Validators.pattern(/^[a-z0-9_-]+$/)]],
      display_name: ['', [Validators.required, Validators.maxLength(100)]],
      api_base: ['', [Validators.required, Validators.pattern(/^https?:\/\/.+/)]],
      model_name: ['', [Validators.required, Validators.maxLength(100)]],
      api_key: ['', this.mode === 'create' ? [Validators.required] : []],
      max_tokens: [4096, [Validators.required, Validators.min(1), Validators.max(128000)]],
      temperature: [0.7, [Validators.required, Validators.min(0), Validators.max(2)]],
      timeout_seconds: [60, [Validators.required, Validators.min(5), Validators.max(600)]],
      priority: [0, [Validators.required, Validators.min(0), Validators.max(100)]],
    });
  }

  ngOnInit(): void {
    if (this.mode === 'edit' && this.provider) {
      this.form.patchValue({
        name: this.provider.name,
        display_name: this.provider.display_name,
        api_base: this.provider.api_base,
        model_name: this.provider.model_name,
        max_tokens: this.provider.max_tokens,
        temperature: this.provider.temperature,
        timeout_seconds: this.provider.timeout_seconds,
        priority: this.provider.priority,
      });
      // Name is not editable in edit mode
      this.form.get('name')?.disable();
      // API key is optional in edit mode
      this.form.get('api_key')?.clearValidators();
      this.form.get('api_key')?.updateValueAndValidity();
    }
  }

  applyPreset(preset: any): void {
    if (!preset || preset.name === 'custom') return;
    this.form.patchValue({
      name: preset.name,
      display_name: preset.display,
      api_base: preset.api_base,
      model_name: preset.model,
    });
  }

  toggleApiKeyVisibility(): void {
    this.showApiKey = !this.showApiKey;
  }

  submit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }

    const formValue = this.form.getRawValue();

    const result: LLMProviderFormDialogResult = {
      name: formValue.name,
      display_name: formValue.display_name,
      api_base: formValue.api_base,
      model_name: formValue.model_name,
      max_tokens: formValue.max_tokens,
      temperature: formValue.temperature,
      timeout_seconds: formValue.timeout_seconds,
      priority: formValue.priority,
    };

    // Only include api_key if provided
    if (formValue.api_key) {
      result.api_key = formValue.api_key;
    }

    this.dialogRef.close(result);
  }

  cancel(): void {
    this.dialogRef.close();
  }
}
