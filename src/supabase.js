import { createClient } from '@supabase/supabase-js';

const supabaseUrl = 'https://mjbqlrzcijxlontrbhak.supabase.co';
const supabaseKey = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Im1qYnFscnpjaWp4bG9udHJiaGFrIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzkxOTA0MTcsImV4cCI6MjA5NDc2NjQxN30.exvRp1J7iEgs7qXTePe1Mi9dcQfUli8PIOdlteWJa6M';

export const supabase = createClient(supabaseUrl, supabaseKey, {
  auth: {
    flowType: 'pkce',
    autoRefreshToken: true,
    persistSession: true,
    detectSessionInUrl: true
  }
});
