// Debug Dashboard Alpine.js Component
function debugDashboard() {
    return {
        // State
        requests: [],
        filteredRequests: [],
        selectedRequest: null,
        autoRefresh: true,
        autoRefreshInterval: null,
        searchQuery: '',
        typeFilter: 'ALL', // 'ALL', 'USER', 'TOOL'

        // Lifecycle
        init() {
            console.log('🔍 Debug Dashboard initialized');
            this.refreshRequests();
            this.toggleAutoRefresh();
        },

        // API Methods
        async refreshRequests() {
            try {
                console.log('🔄 Refreshing requests...');
                const response = await fetch('/api/requests');
                const data = await response.json();
                
                // Only update requests if data has changed
                if (JSON.stringify(data) !== JSON.stringify(this.requests)) {
                    this.requests = data.sort((a, b) => b.timestamp - a.timestamp);
                    this.filterRequests(); // Apply current search filter
                    console.log(`📊 Updated with ${this.requests.length} requests`);
                    
                    // If we had a selected request, try to keep it selected
                    if (this.selectedRequest) {
                        const stillExists = this.requests.find(r => r.id === this.selectedRequest.id);
                        if (!stillExists) {
                            this.selectedRequest = null;
                        }
                    }
                }
            } catch (error) {
                console.error('❌ Failed to refresh requests:', error);
            }
        },

        async clearRequests() {
            try {
                console.log('🧹 Clearing all requests...');
                await fetch('/api/clear', { method: 'POST' });
                this.requests = [];
                this.filteredRequests = [];
                this.selectedRequest = null;
                console.log('✅ Requests cleared');
            } catch (error) {
                console.error('❌ Failed to clear requests:', error);
            }
        },

        // Search and Filter Methods
        filterRequests() {
            let filtered = [...this.requests];
            
            // Apply type filter
            if (this.typeFilter !== 'ALL') {
                filtered = filtered.filter(request => this.getRequestType(request) === this.typeFilter);
            }
            
            // Apply search filter
            if (this.searchQuery.trim()) {
                const query = this.searchQuery.toLowerCase();
                filtered = filtered.filter(request => {
                    // Search in JSON content
                    const jsonString = JSON.stringify(request).toLowerCase();
                    if (jsonString.includes(query)) return true;
                    
                    // Also search in extracted user message
                    const userMessage = this.extractUserMessage(request);
                    if (userMessage && userMessage.toLowerCase().includes(query)) return true;
                    
                    return false;
                });
            }
            
            this.filteredRequests = filtered;
            console.log(`🔍 Filtered ${this.filteredRequests.length}/${this.requests.length} requests (search: "${this.searchQuery}", type: ${this.typeFilter})`);
        },

        setTypeFilter(type) {
            this.typeFilter = type;
            this.filterRequests();
        },

        clearSearch() {
            this.searchQuery = '';
            this.filterRequests();
        },

        // UI Methods
        selectRequest(request) {
            this.selectedRequest = request;
            console.log('👆 Selected request:', request.id);
        },

        toggleAutoRefresh() {
            if (this.autoRefresh) {
                console.log('🔄 Starting auto-refresh (5s interval)');
                this.autoRefreshInterval = setInterval(() => {
                    this.refreshRequests();
                }, 5000);
            } else {
                console.log('⏸️ Stopping auto-refresh');
                if (this.autoRefreshInterval) {
                    clearInterval(this.autoRefreshInterval);
                    this.autoRefreshInterval = null;
                }
            }
        },

        // Utility Methods
        formatTime(timestamp) {
            return new Date(timestamp).toLocaleTimeString();
        },

        formatJson(obj) {
            try {
                return JSON.stringify(obj, null, 2);
            } catch (error) {
                return 'Error formatting JSON: ' + error.message;
            }
        },

        highlightSearchTerms(text, searchQuery) {
            if (!searchQuery || !searchQuery.trim()) {
                return text;
            }

            // Escape special regex characters in search query
            const escapedQuery = searchQuery.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
            const regex = new RegExp(`(${escapedQuery})`, 'gi');
            
            // Replace matches with highlighted spans
            return text.replace(regex, '<span class="search-highlight">$1</span>');
        },

        syntaxHighlight(obj, searchQuery = '') {
            try {
                const jsonString = JSON.stringify(obj, null, 2);
                
                // Apply syntax highlighting
                let highlighted = jsonString
                    // Strings (including keys)
                    .replace(/"([^"\\]*(\\.[^"\\]*)*)"/g, (match, content) => {
                        // Check if this is a key (followed by colon) or value
                        const isKey = jsonString.indexOf(match + ':') !== -1;
                        const className = isKey ? 'json-key' : 'json-string';
                        return `<span class="${className}">"${content}"</span>`;
                    })
                    // Numbers
                    .replace(/:\s*(-?\d+\.?\d*)/g, ': <span class="json-number">$1</span>')
                    // Booleans
                    .replace(/:\s*(true|false)/g, ': <span class="json-boolean">$1</span>')
                    // Null
                    .replace(/:\s*(null)/g, ': <span class="json-null">$1</span>')
                    // Punctuation
                    .replace(/([{}[\],:])/g, '<span class="json-punctuation">$1</span>');

                // Apply search highlighting on top of syntax highlighting
                if (searchQuery && searchQuery.trim()) {
                    const escapedQuery = searchQuery.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
                    const regex = new RegExp(`(${escapedQuery})`, 'gi');
                    highlighted = highlighted.replace(regex, '<span class="search-highlight">$1</span>');
                }

                return highlighted;
            } catch (error) {
                return 'Error formatting JSON: ' + error.message;
            }
        },

        extractUserMessage(request) {
            try {
                let message = null;
                
                // Method 1: Try to extract from currentMessage structure
                if (request.body && request.body.unredacted_input) {
                    const input = request.body.unredacted_input;
                    
                    // Look for currentMessage -> userInputMessage -> content
                    if (input.conversationState && 
                        input.conversationState.currentMessage && 
                        input.conversationState.currentMessage.userInputMessage && 
                        input.conversationState.currentMessage.userInputMessage.content) {
                        message = input.conversationState.currentMessage.userInputMessage.content;
                    }
                }
                
                // Method 2: If not found, try the USER MESSAGE pattern in the entire JSON
                if (!message) {
                    const jsonString = JSON.stringify(request);
                    
                    // Find the outermost USER MESSAGE block
                    const userMessageRegex = /--- USER MESSAGE BEGIN ---\\n(.*?)\\n--- USER MESSAGE END ---/s;
                    const match = jsonString.match(userMessageRegex);
                    
                    if (match && match[1]) {
                        message = match[1]
                            .replace(/\\n/g, ' ')  // Replace newlines with spaces
                            .replace(/\\"/g, '"')  // Unescape quotes
                            .replace(/\\\\/g, '\\') // Unescape backslashes
                            .trim();
                    }
                }
                
                // Clean up the message if found
                if (message) {
                    message = message.trim();
                    
                    // Remove any embedded context entries
                    message = message.replace(/--- CONTEXT ENTRY BEGIN ---.*?--- CONTEXT ENTRY END ---/gs, '').trim();
                    
                    // Remove any embedded USER MESSAGE tags (but keep the content)
                    message = message.replace(/--- USER MESSAGE BEGIN ---/g, '');
                    message = message.replace(/--- USER MESSAGE END ---/g, '');
                    
                    // Clean up extra whitespace
                    message = message.replace(/\s+/g, ' ').trim();
                    
                    // Truncate if too long (for display in snippet)
                    if (message.length > 100) {
                        message = message.substring(0, 97) + '...';
                    }
                    
                    return message || null;
                }
                
                return null;
            } catch (error) {
                console.error('Error extracting user message:', error);
                return null;
            }
        },

        getRequestType(request) {
            const userMessage = this.extractUserMessage(request);
            return userMessage && userMessage.trim() ? 'USER' : 'TOOL';
        },

        async copyRequestData(request) {
            try {
                const text = this.formatJson(request);
                await navigator.clipboard.writeText(text);
                console.log('📋 Request data copied to clipboard');
                
                // Show a brief success indicator
                this.showCopySuccess();
            } catch (error) {
                console.error('❌ Failed to copy request data:', error);
                // Fallback for older browsers
                this.fallbackCopy(this.formatJson(request));
            }
        },

        showCopySuccess() {
            // Create a temporary success message
            const button = event.target;
            const originalText = button.textContent;
            button.textContent = 'Copied!';
            button.style.background = '#28a745';
            
            setTimeout(() => {
                button.textContent = originalText;
                button.style.background = '';
            }, 1000);
        },

        fallbackCopy(text) {
            // Fallback copy method for older browsers
            const textArea = document.createElement('textarea');
            textArea.value = text;
            document.body.appendChild(textArea);
            textArea.select();
            try {
                document.execCommand('copy');
                console.log('📋 Request data copied (fallback method)');
            } catch (error) {
                console.error('❌ Fallback copy failed:', error);
            }
            document.body.removeChild(textArea);
        }
    };
}

// Global utility functions (if needed)
window.debugUtils = {
    formatBytes(bytes) {
        if (bytes === 0) return '0 Bytes';
        const k = 1024;
        const sizes = ['Bytes', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    },

    formatDuration(ms) {
        if (ms < 1000) return `${ms}ms`;
        if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
        return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
    }
};

console.log('🚀 Debug Dashboard scripts loaded');
