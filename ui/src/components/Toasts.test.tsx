import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, renderHook, act } from '@testing-library/react';
import React from 'react';
import { Toasts, type Toast } from './Toasts';
import { useToasts } from '../hooks/useToasts';

describe('Toasts component', () => {
  it('renders toasts and dismiss button calls onDismiss', () => {
    const onDismiss = vi.fn();
    const toasts: Toast[] = [
      { id: 1, text: 'Lỗi mạng', kind: 'error' },
      { id: 2, text: 'Xong', kind: 'success' },
    ];
    render(<Toasts toasts={toasts} onDismiss={onDismiss} />);
    expect(screen.getByText('Lỗi mạng')).toBeDefined();
    expect(screen.getByText('Xong')).toBeDefined();
    fireEvent.click(screen.getAllByLabelText('Đóng thông báo')[0]);
    expect(onDismiss).toHaveBeenCalledWith(1);
  });
});

describe('useToasts', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('pushes, auto-dismisses after 4s, caps at 4', () => {
    const { result } = renderHook(() => useToasts());
    act(() => result.current.pushToast('một', 'info'));
    expect(result.current.toasts).toHaveLength(1);
    expect(result.current.toasts[0].text).toBe('một');

    act(() => {
      for (let i = 0; i < 5; i++) result.current.pushToast(`t${i}`);
    });
    expect(result.current.toasts).toHaveLength(4);

    act(() => vi.advanceTimersByTime(4100));
    expect(result.current.toasts).toHaveLength(0);
  });

  it('dismissToast removes immediately and clears timer', () => {
    const { result } = renderHook(() => useToasts());
    act(() => result.current.pushToast('x'));
    const id = result.current.toasts[0].id;
    act(() => result.current.dismissToast(id));
    expect(result.current.toasts).toHaveLength(0);
    act(() => vi.advanceTimersByTime(10000));
    expect(result.current.toasts).toHaveLength(0);
  });
});
