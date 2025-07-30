<script setup lang="ts">
import {ref} from "vue";
import SessionManager from "../../managers/session.ts";

const user = ref(SessionManager.getUser()!);
const token = ref("");
const loadingToken = ref(false);
const linkingError = ref("");

function downloadMyData() {

}

function deleteAccount() {
  confirm("Are you sure you want to delete your account and all data associated with it? This action cannot be undone.");
}

async function copyToken() {
  if (token.value !== "") {
    await navigator.clipboard.writeText(token.value);
    return;
  }

  if (loadingToken.value) return;
  loadingToken.value = true;

  try {
    const response = await fetch('/auth/link');
    const data = await response.json();

    if (!response.ok) {
      throw new Error(data.message || 'Unknown error occurred while linking account');
    }

    token.value = data.token;
    await navigator.clipboard.writeText(token.value);
  } catch (error) {
    console.error("Error linking account:", error);
    linkingError.value = error instanceof Error ? error.message : 'An unknown error occurred';
  } finally {
    loadingToken.value = false;
  }
}

function verifyAccount() {
  SessionManager.validateSession().then(() => {
    const userData = SessionManager.getUser();
    if (userData) {
      user.value = userData;
      if (user.value.account_id === -1) {
        linkingError.value = "Account linking failed. Please try again.";
      } else {
        linkingError.value = "";
      }
    }
  })
}

</script>

<template>
  <div>
    <section v-if="user.account_id == -1">
      <div class="warning-info">
        Please link your <b>Geometry Dash</b> account below
      </div>
      <p class="flex-3">
        To link your account, press the "Link Account" button, which will copy your secret token to the clipboard.
        Then, open Geometry Dash, go to Level Thumbnails settings, and paste the token into the "Link Account" form.
        <br/>
        After that, return to this page and click the "Verify Account" button to check if the linking was successful.
      </p>
      <div class="d-flex">
        <input type="text" v-model="token" class="flex-3 link-token" placeholder="Your secret token will appear here"
               readonly/>
        <button @click="copyToken" class="secondary-bg flex-1 link-account" :disabled="loadingToken">
          {{ token ? "Copy Token" : "Link Account" }}
        </button>
      </div>
      <button @click="verifyAccount" class="success-bg w-100 mt-1">
        Verify Account
      </button>
      <p v-if="linkingError" class="error-message">
        {{ linkingError }}
      </p>
    </section>
    <section>
      <h3>Privacy & Data</h3>
      <p class="note">
        We store your Geometry Dash account ID, Discord ID, and any thumbnails you upload.
        Learn more in our <a href="/privacy" target="_blank">Privacy Policy</a>.
      </p>
      <div class="d-flex gap-1">
        <button @click="downloadMyData" class="primary-bg flex-1">
          Request My Data
        </button>
        <button @click="deleteAccount" class="danger-bg flex-1">
          Delete My Account
        </button>
      </div>
    </section>
    <section class="mobile-only">
      <button @click="SessionManager.logout()" class="logout-button w-100">
        <img src="/icons/logout.svg" alt="Logout" class="avatar"/>
        Logout
      </button>
    </section>
  </div>
</template>

<style scoped>
section {
  background-color: rgba(0, 0, 0, 0.25);
  padding: 20px;
  border-radius: 12px;
  margin-bottom: 20px;
}

h3 {
  margin: 0 0 10px 0;
  color: #fff;
}

.note {
  color: #aaa;
  font-size: 0.9em;
}

.link-token {
  background-color: #fff;
  color: #000;
  border: 0;
  border-radius: 8px 0 0 8px;
  padding: 10px;
  font-size: 1em;
}

.link-account {
  border: none;
  border-radius: 0 8px 8px 0;
  padding: 10px;
  font-size: 1em;
}

button {
  background-color: #000;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  border: none;
  padding: 10px 20px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 1em;
  transition: background-color 0.3s ease;
}

button > img {
  width: 24px;
  height: 24px;
  margin-right: 10px;
}

.primary-bg {
  background-color: #007bff;
}

.danger-bg {
  background-color: #dc3545;
}

.secondary-bg {
  background-color: #6c757d;
}

.success-bg {
  background-color: #28a745;
}

button:hover {
  background-color: #0056b3;
}

button.danger-bg:hover {
  background-color: #c82333;
}

button.secondary-bg:hover {
  background-color: #5a6268;
}

button.success-bg:hover {
  background-color: #218838;
}
</style>