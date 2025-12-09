import { NbToastrService } from '@nebular/theme';

export class ErrorHandler {
  static extractErrorMessage(error: any): string {
    if (!error) return '操作失败，请稍后重试';

    // Handle string error directly
    if (typeof error === 'string') return error;

    // HttpErrorResponse: error.error contains the response body
    let body = error.error;
    
    // If body is a string, try to parse it as JSON
    if (typeof body === 'string') {
      try {
        body = JSON.parse(body);
      } catch (e) {
        return body; // plain string error message
      }
    }
    
    // Backend returns {code, message} format
    if (body && typeof body === 'object' && body.message) {
      return body.message;
    }
    
    // Nested error structure
    if (body?.error?.message) {
      return body.error.message;
      }
      
    // Direct message on error object (but not Angular's generic Http failure message)
    if (error.message && !error.message.includes('Http failure')) {
        return error.message;
      }
      
    // Status code based message
      if (error.status) {
        return this.getDefaultMessageByStatus(error.status);
    }
    
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

  /**
   * Handle HTTP error and show toast notification
   */
  static handleHttpError(error: any, toastrService: NbToastrService): void {
    const errorMsg = this.extractErrorMessage(error);
    toastrService.danger(errorMsg, '错误', { duration: 5000 });
  }
}
