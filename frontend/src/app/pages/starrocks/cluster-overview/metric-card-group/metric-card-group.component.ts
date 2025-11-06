import { Component, Input, OnInit } from '@angular/core';
import { trigger, transition, style, animate } from '@angular/animations';

@Component({
  selector: 'ngx-metric-card-group',
  templateUrl: './metric-card-group.component.html',
  styleUrls: ['./metric-card-group.component.scss'],
  animations: [
    trigger('slideDown', [
      transition(':enter', [
        style({ height: 0, opacity: 0, overflow: 'hidden' }),
        animate('300ms ease-out', style({ height: '*', opacity: 1 })),
      ]),
      transition(':leave', [
        animate('300ms ease-in', style({ height: 0, opacity: 0, overflow: 'hidden' })),
      ]),
    ]),
  ],
})
export class MetricCardGroupComponent implements OnInit {
  @Input() title: string;
  @Input() icon: string;
  @Input() collapsed: boolean = false;
  @Input() loading: boolean = false;
  @Input() badgeCount: number = 0;
  @Input() badgeStatus: string = 'danger'; // danger, warning, info
  
  toggleCollapse(event?: Event) {
    if (event) {
      event.stopPropagation();
    }
    this.collapsed = !this.collapsed;
    // Save collapse state to localStorage
    this.saveCollapseState();
  }

  private saveCollapseState() {
    const key = `metric_group_${this.title}_collapsed`;
    localStorage.setItem(key, String(this.collapsed));
  }

  ngOnInit(): void {
    // Load collapse state from localStorage
    const key = `metric_group_${this.title}_collapsed`;
    const saved = localStorage.getItem(key);
    if (saved !== null) {
      this.collapsed = saved === 'true';
    }
  }
}

