import { errorMessage } from './i18n';

/** The shared service error envelope, independent of either host transport. */
export class ApiError extends Error {
  constructor(
    public status: number,
    public errorCode: string,
    public category: string,
    message: string,
    public details?: { request_id?: string; expires_at?: number; resend_after?: number },
  ) {
    super(errorMessage(message, errorCode));
    this.name = 'ApiError';
  }
}
