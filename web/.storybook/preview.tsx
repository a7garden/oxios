import type { Preview } from '@storybook/tanstack-react'
import { withThemeByClassName } from '@storybook/addon-themes'

import '../src/index.css'

const preview: Preview = {
  parameters: {
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
    a11y: {
      test: 'todo',
    },
    layout: 'centered',
    docs: {
      toc: true,
    },
    viewport: {
      viewports: {
        mobileMin: { name: 'Mobile (360)', styles: { width: '360px', height: '640px' } },
        mobile: { name: 'Mobile (375)', styles: { width: '375px', height: '812px' } },
        notched: { name: 'Notched (390)', styles: { width: '390px', height: '844px' } },
        tablet: { name: 'Tablet (768)', styles: { width: '768px', height: '1024px' } },
        desktop: { name: 'Desktop (1280)', styles: { width: '1280px', height: '900px' } },
      },
    },
  },
  tags: ['autodocs'],
  decorators: [
    withThemeByClassName({
      themes: {
        light: '',
        dark: 'dark',
      },
      defaultTheme: 'light',
    }),
  ],
}

export default preview
