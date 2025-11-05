import { HttpErrorResponse } from '@angular/common/http';

export class ErrorHandler {
  static extractErrorMessage(error: any): string {
    console.log('[ErrorHandler] Full error object:', error);
    console.log('[ErrorHandler] Error type:', typeof error);
    console.log('[ErrorHandler] Error keys:', error ? Object.keys(error) : 'null/undefined');
    console.log('[ErrorHandler] Error constructor:', error?.constructor?.name);
    console.log('[ErrorHandler] Is HttpErrorResponse:', error instanceof HttpErrorResponse);
    
    // 处理Angular HttpErrorResponse
    if (error && typeof error === 'object') {
      // 检查是否有后端返回的message（嵌套结构）
      if (error.error && typeof error.error === 'object' && error.error.message) {
        console.log('[ErrorHandler] Using error.error.message:', error.error.message);
        return error.error.message;
      }
      
      // 检查是否有嵌套的错误信息
      if (error.error && error.error.error && error.error.error.message) {
        console.log('[ErrorHandler] Using nested error.message:', error.error.error.message);
        return error.error.error.message;
      }
      
      // 检查是否有直接的message（后端直接返回的结构）
      if (error.message) {
        console.log('[ErrorHandler] Using error.message:', error.message);
        return error.message;
      }
      
      // 检查是否有statusText
      if (error.statusText) {
        console.log('[ErrorHandler] Using error.statusText:', error.statusText);
        return error.statusText;
      }
      
      // HTTP状态码对应的默认消息
      if (error.status) {
        console.log('[ErrorHandler] Using status-based message for status:', error.status);
        return this.getDefaultMessageByStatus(error.status);
      }
    }
    
    console.log('[ErrorHandler] Using fallback message');
    return '操作失败，请稍后重试';
  }
  
  /**
   * Handle cluster-related errors and return user-friendly message
   * 处理集群相关错误，统一返回用户友好的错误提示
   */
  static handleClusterError(error: any): string {
    const errorMsg = this.extractErrorMessage(error);
    
    // If error message mentions "No active cluster" or similar, return unified message
    if (errorMsg.includes('No active cluster') || 
        errorMsg.includes('没有激活') ||
        (error.status === 404 && errorMsg.includes('System function'))) {
      return '请先激活一个集群';
    }
    
    // If it's a 404, likely a cluster issue
    if (error.status === 404) {
      return '请先激活一个集群';
    }
    
    return errorMsg;
  }
  
  private static getDefaultMessageByStatus(status: number): string {
    const statusMessages: { [key: number]: string } = {
      400: '请求参数有误',
      401: '没有权限执行此操作',
      403: '没有权限执行此操作',
      404: '请求的资源不存在',
      500: '服务器内部错误',
      503: '服务暂时不可用'
    };
    return statusMessages[status] || '网络请求失败';
  }
}
