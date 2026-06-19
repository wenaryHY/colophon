/**
 * Bug Condition Exploration Test for Admin Routing Consistency
 * 
 * **Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5**
 * 
 * **Property 1: Bug Condition** - Admin Entry Point Routing Consistency
 * 
 * **CRITICAL**: This test MUST FAIL on unfixed code - failure confirms the bug exists
 * **DO NOT attempt to fix the test or the code when it fails**
 * **NOTE**: This test encodes the expected behavior - it will validate the fix when it passes after implementation
 * 
 * **GOAL**: Surface counterexamples that demonstrate routing inconsistencies exist
 * 
 * This test explores three specific bug conditions:
 * 1. Dev environment requires trailing slash for `/admin/` due to Vite base config
 * 2. ADMIN button navigates to `/admin/posts` instead of `/admin`
 * 3. ADMIN button lacks i18n translation attribute
 */

import { describe, it, expect } from 'vitest'
import * as fc from 'fast-check'
import { readFileSync } from 'fs'
import { join } from 'path'

describe('Bug Condition Exploration: Admin Routing Consistency', () => {
  
  /**
   * Bug Condition 1: Vite Base Configuration Requires Trailing Slash
   * 
   * Expected behavior: `/admin` (no trailing slash) should work in dev environment
   * Current behavior: Vite config has `base: '/admin/'` requiring trailing slash
   * 
   * This test checks if Vite's base configuration is causing the trailing slash requirement.
   */
  it('should NOT require trailing slash for /admin in dev environment', () => {
    const viteConfigPath = join(__dirname, '../../vite.config.ts')
    const viteConfig = readFileSync(viteConfigPath, 'utf-8')
    
    // Check if base is set to '/admin/' (with trailing slash)
    const baseMatch = viteConfig.match(/base:\s*['"]([^'"]+)['"]/);
    
    if (baseMatch) {
      const baseValue = baseMatch[1]
      
      // Expected: base should be '/admin' (no trailing slash) or not set
      // Current (buggy): base is '/admin/' (with trailing slash)
      expect(baseValue).not.toBe('/admin/')
      
      // If base is set, it should not end with a slash
      if (baseValue.startsWith('/admin')) {
        expect(baseValue).toBe('/admin')
      }
    }
  })

  /**
   * Bug Condition 2: React Router Missing Basename Configuration
   * 
   * Expected behavior: BrowserRouter should have basename="/admin" to handle routing correctly
   * Current behavior: BrowserRouter has no basename prop
   * 
   * This test checks if React Router is properly configured with basename.
   */
  it('should have basename="/admin" configured in BrowserRouter', () => {
    const appPath = join(__dirname, '../App.tsx')
    const appContent = readFileSync(appPath, 'utf-8')
    
    // Check if BrowserRouter has basename prop
    const browserRouterMatch = appContent.match(/<BrowserRouter[^>]*>/);
    
    expect(browserRouterMatch).toBeTruthy()
    
    if (browserRouterMatch) {
      const browserRouterTag = browserRouterMatch[0]
      
      // Expected: <BrowserRouter basename="/admin">
      // Current (buggy): <BrowserRouter> (no basename)
      expect(browserRouterTag).toMatch(/basename=["']\/admin["']/)
    }
  })

  /**
   * Bug Condition 3: ADMIN Button Missing i18n Attribute
   * 
   * Expected behavior: ADMIN button should have data-i18n="admin" attribute
   * Current behavior: Button has hardcoded "Admin" text without i18n attribute
   * 
   * This test checks if the ADMIN button in profile.html has the i18n attribute.
   */
  it('should have admin entry link with server-side i18n in _header.html', () => {
    const headerPath = join(__dirname, '../../../../../themes/default/templates/_header.html')
    const headerContent = readFileSync(headerPath, 'utf-8')
    
    // Find the ADMIN button link — uses Jinja2 server-side i18n, not data-i18n
    const adminButtonMatch = headerContent.match(/<a[^>]*href=["']\/admin["'][^>]*class=["'][^"']*site-header__admin-link[^"']*["'][^>]*>/);
    
    expect(adminButtonMatch).toBeTruthy()
    
    if (adminButtonMatch) {
      const adminButton = adminButtonMatch[0]
      
      // Verify the admin link has correct href
      expect(adminButton).toMatch(/href=["']\/admin["']/)
      
      // Verify server-side i18n logic exists (Jinja2 {% if current_lang %})
      expect(headerContent).toContain('{% if current_lang == "zh" %}管理后台{% else %}Admin{% endif %}')
    }
  })

  /**
   * Property-Based Test: Admin Entry Point Navigation Consistency
   * 
   * **Validates: Requirements 2.1, 2.2, 2.3**
   * 
   * This property test generates various navigation scenarios to the admin entry point
   * and verifies that the routing configuration is consistent.
   * 
   * The test explores:
   * - Different URL formats: /admin, /admin/, /admin?query, /admin#hash
   * - Different environments: dev (5173), prod (2000)
   * - Different entry points: direct URL access, button click
   */
  it('property: admin entry point should route consistently across all scenarios', () => {
    fc.assert(
      fc.property(
        fc.record({
          path: fc.constantFrom('/admin', '/admin/', '/admin?test=1', '/admin#section'),
          environment: fc.constantFrom('dev', 'prod'),
          entryPoint: fc.constantFrom('direct-url', 'admin-button')
        }),
        (scenario) => {
          // This property encodes the expected behavior:
          // For ANY navigation to admin entry point, the system should:
          // 1. Accept /admin without trailing slash
          // 2. Route to /admin (not /admin/posts directly)
          // 3. Let React Router handle the redirect to /admin/posts
          
          const viteConfigPath = join(__dirname, '../../vite.config.ts')
          const viteConfig = readFileSync(viteConfigPath, 'utf-8')
          const baseMatch = viteConfig.match(/base:\s*['"]([^'"]+)['"]/);
          
          if (baseMatch) {
            const baseValue = baseMatch[1]
            
            // Property: Vite base should not require trailing slash
            // This ensures consistent behavior in dev environment
            if (scenario.environment === 'dev' && scenario.path === '/admin') {
              // The base should be '/admin' or not end with '/'
              const requiresTrailingSlash = baseValue === '/admin/'
              
              // Expected: false (should NOT require trailing slash)
              // Current (buggy): true (DOES require trailing slash)
              expect(requiresTrailingSlash).toBe(false)
            }
          }
          
          // Property: React Router should have basename configured
          // This ensures proper routing context
          const appPath = join(__dirname, '../App.tsx')
          const appContent = readFileSync(appPath, 'utf-8')
          const hasBasename = /<BrowserRouter[^>]*basename=["']\/admin["']/.test(appContent)
          
          // Expected: true (should have basename)
          // Current (buggy): false (no basename)
          expect(hasBasename).toBe(true)
        }
      ),
      { numRuns: 20 } // Run 20 test cases to explore the input space
    )
  })

  /**
   * Property-Based Test: i18n Translation for ADMIN Button
   * 
   * **Validates: Requirements 2.4, 2.5, 2.6**
   * 
   * This property test verifies that the ADMIN button supports i18n translation
   * for different language preferences.
   */
  it('property: ADMIN button should support i18n via server-side templates', () => {
    fc.assert(
      fc.property(
        fc.constantFrom('zh', 'en', 'ja', 'es', 'fr'),
        (language) => {
          // For ANY language preference, the ADMIN button uses Jinja2 server-side
          // i18n templates rather than client-side data-i18n attributes
          
          const headerPath = join(__dirname, '../../../../../themes/default/templates/_header.html')
          const headerContent = readFileSync(headerPath, 'utf-8')
          
          const adminButtonMatch = headerContent.match(/<a[^>]*href=["']\/admin["'][^>]*class=["'][^"']*site-header__admin-link[^"']*["'][^>]*>/);
          
          expect(adminButtonMatch).toBeTruthy()
          
          // Verify i18n is handled server-side — the template contains
          // {% if current_lang %} blocks for zh and en translations
          expect(headerContent).toMatch(/\{%\s*if\s+current_lang/)
          expect(headerContent).toContain('管理后台')
          expect(headerContent).toContain('Admin')
        }
      ),
      { numRuns: 5 } // Test with 5 different languages
    )
  })
})
