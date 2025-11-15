import { NgModule } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TranslateModule } from '@ngx-translate/core';
import { 
  NbCardModule, 
  NbInputModule, 
  NbButtonModule, 
  NbIconModule, 
  NbSpinnerModule,
  NbAlertModule,
  NbTooltipModule,
  NbRadioModule
} from '@nebular/theme';

import { UserSettingsComponent } from './user-settings.component';
import { RouterModule } from '@angular/router';

@NgModule({
  declarations: [
    UserSettingsComponent
  ],
  imports: [
    CommonModule,
    FormsModule,
    TranslateModule,
    NbCardModule,
    NbInputModule,
    NbButtonModule,
    NbAlertModule,
    NbSpinnerModule,
    NbIconModule,
    NbTooltipModule,
    NbRadioModule,
    RouterModule.forChild([
      {
        path: '',
        component: UserSettingsComponent
      }
    ])
  ]
})
export class UserSettingsModule { }

