import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// Mock axios before importing api
vi.mock('axios');

import axios from 'axios';
import { api } from './api';

const mockedAxios = axios as any;

describe('InferstructorAPI', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    vi.clearAllMocks();
  });

  afterEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    api.logout();
    api.adminLogout();
  });

  describe('Authentication', () => {
    it('should register a new validator', async () => {
      const mockResponse = {
        data: {
          success: true,
          credentials: {
            validator_id: 'val_123',
            chain: 'solana',
            api_key: 'key_123',
            api_secret: 'secret_123',
            sla_tier: 'pro',
            max_tps: 1000,
            bridge_endpoint: 'http://bridge.local',
            toll_booth_endpoint: 'http://tollbooth.local',
            jwt_token: 'jwt_token_123',
          },
        },
      };
      mockedAxios.post.mockResolvedValueOnce(mockResponse);

      const result = await api.register('solana', 'user@example.com', 'pro');

      expect(result.validator_id).toBe('val_123');
      expect(result.api_key).toBe('key_123');
    });

    it('should throw error on registration failure', async () => {
      mockedAxios.post.mockResolvedValueOnce({
        data: { success: false },
      });

      await expect(api.register('solana', 'user@example.com')).rejects.toThrow('Registration failed');
    });

    it('should login with credentials', async () => {
      const mockResponse = {
        data: {
          success: true,
          token: 'jwt_token_456',
          validator: { id: 'val_456' },
        },
      };
      mockedAxios.post.mockResolvedValueOnce(mockResponse);

      await api.login('key_456', 'secret_456');

      // JWT should be stored in sessionStorage (not localStorage)
      expect(sessionStorage.getItem('infra_jwt_token')).toBe('jwt_token_456');
      // API key should NOT be persisted (in memory only)
      expect(api.getAPIKey()).toBe('key_456');
    });

    it('should logout and clear credentials', () => {
      sessionStorage.setItem('infra_jwt_token', 'token_123');
      sessionStorage.setItem('infra_validator_id', 'val_123');

      api.logout();

      expect(sessionStorage.getItem('infra_jwt_token')).toBeNull();
      // API key is memory-only, no need to check storage
      expect(api.getAPIKey()).toBeNull();
    });
  });

  describe('Admin Operations', () => {
    it('should admin login', async () => {
      // Mock CSRF token fetch
      const csrfMockResponse = {
        data: {
          token: 'csrf_token_123',
        },
      };
      // Mock admin login response
      const loginMockResponse = {
        data: {
          success: true,
          token: 'admin_token_123',
          expires_in: 3600,
        },
      };
      mockedAxios.get.mockResolvedValueOnce(csrfMockResponse);
      mockedAxios.post.mockResolvedValueOnce(loginMockResponse);

      const result = await api.adminLogin('password123');
      expect(result.token).toBe('admin_token_123');
    });

    it('should admin logout', () => {
      sessionStorage.setItem('infra_admin_token', 'admin_token_test');
      api.adminLogout();
      expect(sessionStorage.getItem('infra_admin_token')).toBeNull();
    });
  });

  describe('Error Handling', () => {
    it('should throw error if not authenticated when getting stats', async () => {
      localStorage.clear();

      await expect(api.getStats()).rejects.toThrow('Not authenticated');
    });

    it('should throw error if no API key when testing connection', async () => {
      localStorage.clear();

      await expect(api.testConnection()).rejects.toThrow('No API key');
    });
  });
});
