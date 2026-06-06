import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ServerCard } from '@/components/mcp/server-card'
import type { McpServer } from '@/types/mcp'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

const connectedServer: McpServer = {
  name: 'test-server',
  command: 'npx',
  args: ['-y', '@modelcontextprotocol/server-filesystem', '/tmp'],
  enabled: true,
  initialized: true,
}

const disconnectedServer: McpServer = {
  name: 'broken-server',
  command: 'node',
  args: ['server.js'],
  enabled: true,
  initialized: false,
}

const disabledServer: McpServer = {
  name: 'disabled-server',
  command: 'python',
  args: ['mcp_server.py'],
  enabled: false,
  initialized: false,
}

const defaultProps = {
  onToggle: vi.fn(),
  onRefresh: vi.fn(),
  onDelete: vi.fn(),
  isToggling: false,
  isRefreshing: false,
  isDeleting: false,
}

describe('ServerCard', () => {
  it('renders server name', () => {
    render(<ServerCard server={connectedServer} {...defaultProps} />)

    expect(screen.getByText('test-server')).toBeInTheDocument()
  })

  it('renders command with args', () => {
    render(<ServerCard server={connectedServer} {...defaultProps} />)

    expect(
      screen.getByText('npx -y @modelcontextprotocol/server-filesystem /tmp'),
    ).toBeInTheDocument()
  })

  it('renders command without args', () => {
    const noArgsServer = { ...connectedServer, args: [] }
    render(<ServerCard server={noArgsServer} {...defaultProps} />)

    expect(screen.getByText('npx')).toBeInTheDocument()
  })

  it('shows connected state with green dot', () => {
    render(<ServerCard server={connectedServer} {...defaultProps} />)

    expect(screen.getByText('mcp.connected')).toBeInTheDocument()
    const dot = document.querySelector('.bg-success')
    expect(dot).toBeInTheDocument()
  })

  it('shows disconnected state with red dot', () => {
    render(<ServerCard server={disconnectedServer} {...defaultProps} />)

    expect(screen.getByText('mcp.disconnected')).toBeInTheDocument()
    const dot = document.querySelector('.bg-error')
    expect(dot).toBeInTheDocument()
  })

  it('shows disabled state with gray dot', () => {
    render(<ServerCard server={disabledServer} {...defaultProps} />)

    expect(screen.getByText('common.disabled')).toBeInTheDocument()
    const dot = document.querySelector('.bg-gray-400')
    expect(dot).toBeInTheDocument()
  })

  it('calls onToggle when power button is clicked', async () => {
    const onToggle = vi.fn()
    render(<ServerCard server={connectedServer} {...defaultProps} onToggle={onToggle} />)

    const powerBtn = screen.getByTitle('mcp.disable')
    await userEvent.click(powerBtn)

    expect(onToggle).toHaveBeenCalledOnce()
  })

  it('calls onRefresh when refresh button is clicked', async () => {
    const onRefresh = vi.fn()
    render(<ServerCard server={connectedServer} {...defaultProps} onRefresh={onRefresh} />)

    const refreshBtn = screen.getByTitle('mcp.refresh')
    await userEvent.click(refreshBtn)

    expect(onRefresh).toHaveBeenCalledOnce()
  })

  it('calls onDelete when delete button is clicked', async () => {
    const onDelete = vi.fn()
    render(<ServerCard server={connectedServer} {...defaultProps} onDelete={onDelete} />)

    const deleteBtn = screen.getByTitle('mcp.remove')
    await userEvent.click(deleteBtn)

    expect(onDelete).toHaveBeenCalledOnce()
  })

  it('disables refresh button when server is not enabled', () => {
    render(<ServerCard server={disabledServer} {...defaultProps} />)

    const refreshBtn = screen.getByTitle('mcp.refresh')
    expect(refreshBtn).toBeDisabled()
  })

  it('disables toggle button when isToggling', () => {
    render(<ServerCard server={connectedServer} {...defaultProps} isToggling={true} />)

    const powerBtn = screen.getByTitle('mcp.disable')
    expect(powerBtn).toBeDisabled()
  })

  it('disables delete button when isDeleting', () => {
    render(<ServerCard server={connectedServer} {...defaultProps} isDeleting={true} />)

    const deleteBtn = screen.getByTitle('mcp.remove')
    expect(deleteBtn).toBeDisabled()
  })
})
