/* tslint:disable */
/* eslint-disable */
/**
* @param {string} name
*/
export function greet(name: string): void;
/**
* @param {string | undefined} [base_url]
* @param {string | undefined} [pubkey]
* @param {string | undefined} [did_key]
* @param {string | undefined} [private_key]
*/
export function setup_networking_config(base_url?: string, pubkey?: string, did_key?: string, private_key?: string): void;
/**
* @param {string} user_id
* @returns {Promise<void>}
*/
export function initial_user(user_id: string): Promise<void>;
/**
* @param {string} user_id
* @returns {Promise<string>}
*/
export function register_user(user_id: string): Promise<string>;
/**
* @param {string} user_id
* @param {string} group_id
* @returns {Promise<boolean>}
*/
export function is_mls_group(user_id: string, group_id: string): Promise<boolean>;
/**
* @param {string} user_id
* @param {string} group_id
* @returns {Promise<string>}
*/
export function create_group(user_id: string, group_id: string): Promise<string>;
/**
* @param {string} user_id
* @param {(string)[]} group_ids
* @returns {Promise<void>}
*/
export function sync_mls_state(user_id: string, group_ids: (string)[]): Promise<void>;
/**
* @param {string} user_id
* @param {string} target_user_id
* @returns {Promise<boolean>}
*/
export function can_add_member_to_group(user_id: string, target_user_id: string): Promise<boolean>;
/**
* @param {string} user_id
* @param {string} member_user_id
* @param {string} group_id
* @returns {Promise<void>}
*/
export function add_member_to_group(user_id: string, member_user_id: string, group_id: string): Promise<void>;
/**
* @param {string} user_id
* @param {string} msg
* @param {string} group_id
* @returns {Promise<string>}
*/
export function mls_encrypt_msg(user_id: string, msg: string, group_id: string): Promise<string>;
/**
* @param {string} user_id
* @param {string} msg
* @param {string} sender_user_id
* @param {string} group_id
* @returns {Promise<string>}
*/
export function mls_decrypt_msg(user_id: string, msg: string, sender_user_id: string, group_id: string): Promise<string>;
/**
* @param {string} user_id
* @param {Uint8Array} msg_bytes
* @returns {Promise<void>}
*/
export function handle_mls_group_event(user_id: string, msg_bytes: Uint8Array): Promise<void>;
