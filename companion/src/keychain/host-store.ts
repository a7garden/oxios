import * as SecureStore from 'expo-secure-store'
import type { HostProfile } from '../transport/types'
const INDEX='oxios.hosts.index.v1', profileKey=(id:string)=>`oxios.host.${id}.profile.v1`,tokenKey=(id:string)=>`oxios.host.${id}.token.v1`
type Stored=Omit<HostProfile,'deviceToken'>
async function ids():Promise<string[]>{try{const value=await SecureStore.getItemAsync(INDEX);return value?JSON.parse(value):[]}catch{return[]}}
export async function listHostProfiles():Promise<HostProfile[]>{const out:HostProfile[]=[];for(const id of await ids()){const host=await loadHostProfile(id);if(host)out.push(host)}return out}
export async function loadHostProfile(id:string):Promise<HostProfile|null>{const [raw,token]=await Promise.all([SecureStore.getItemAsync(profileKey(id)),SecureStore.getItemAsync(tokenKey(id))]);if(!raw)return null;try{return{...(JSON.parse(raw) as Stored),...(token?{deviceToken:token}:{})}}catch{return null}}
export async function saveHostProfile(host:HostProfile):Promise<void>{const {deviceToken,...stored}=host;await SecureStore.setItemAsync(profileKey(host.id),JSON.stringify(stored));if(deviceToken)await SecureStore.setItemAsync(tokenKey(host.id),deviceToken);const current=await ids();if(!current.includes(host.id))await SecureStore.setItemAsync(INDEX,JSON.stringify([...current,host.id]))}
export async function saveDeviceToken(hostId:string,token:string):Promise<void>{await SecureStore.setItemAsync(tokenKey(hostId),token)}
export async function deleteHostProfile(hostId:string):Promise<void>{await Promise.all([SecureStore.deleteItemAsync(profileKey(hostId)),SecureStore.deleteItemAsync(tokenKey(hostId))]);await SecureStore.setItemAsync(INDEX,JSON.stringify((await ids()).filter(id=>id!==hostId)))}
export const listHosts=listHostProfiles, loadHost=loadHostProfile, saveHost=saveHostProfile, deleteHost=deleteHostProfile
