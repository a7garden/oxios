import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { StatusIndicator } from '@/components/shared/status-indicator'

describe('StatusIndicator', () => {
  it('renders running status with green dot', () => {
    render(<StatusIndicator status="running" />)

    // Component uses CSS capitalize, so DOM text is lowercase
    expect(screen.getByText('running')).toBeInTheDocument()
    const dot = document.querySelector('.bg-emerald-500')
    expect(dot).toBeInTheDocument()
  })

  it('renders active status with green dot', () => {
    render(<StatusIndicator status="active" />)

    expect(screen.getByText('active')).toBeInTheDocument()
    const dot = document.querySelector('.bg-emerald-500')
    expect(dot).toBeInTheDocument()
  })

  it('renders stopped status with gray dot', () => {
    render(<StatusIndicator status="stopped" />)

    expect(screen.getByText('stopped')).toBeInTheDocument()
    const dot = document.querySelector('.bg-zinc-400')
    expect(dot).toBeInTheDocument()
  })

  it('renders pending status with amber dot', () => {
    render(<StatusIndicator status="pending" />)

    expect(screen.getByText('pending')).toBeInTheDocument()
    const dot = document.querySelector('.bg-amber-500')
    expect(dot).toBeInTheDocument()
  })

  it('renders failed status with red dot', () => {
    render(<StatusIndicator status="failed" />)

    expect(screen.getByText('failed')).toBeInTheDocument()
    const dot = document.querySelector('.bg-destructive')
    expect(dot).toBeInTheDocument()
  })

  it('renders error status with destructive dot', () => {
    render(<StatusIndicator status="error" />)

    expect(screen.getByText('error')).toBeInTheDocument()
    const dot = document.querySelector('.bg-destructive')
    expect(dot).toBeInTheDocument()
  })

  it('renders unknown status with default gray dot', () => {
    render(<StatusIndicator status="unknown" />)

    expect(screen.getByText('unknown')).toBeInTheDocument()
    // Unknown status falls back to 'bg-zinc-400'
    const dot = document.querySelector('.bg-zinc-400')
    expect(dot).toBeInTheDocument()
  })

  it('renders idle status with amber dot', () => {
    render(<StatusIndicator status="idle" />)

    expect(screen.getByText('idle')).toBeInTheDocument()
    const dot = document.querySelector('.bg-amber-500')
    expect(dot).toBeInTheDocument()
  })

  it('renders text with capitalize class', () => {
    render(<StatusIndicator status="running" />)

    const text = screen.getByText('running')
    expect(text).toBeInTheDocument()
    expect(text.className).toContain('capitalize')
  })

  it('applies custom className', () => {
    render(<StatusIndicator status="running" className="my-class" />)

    const container = screen.getByText('running').closest('div')
    expect(container?.className).toContain('my-class')
  })
})
